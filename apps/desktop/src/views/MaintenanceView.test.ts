import { render, screen, fireEvent, waitFor, within } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MaintenanceView from './MaintenanceView.svelte';
import maintenanceSource from './MaintenanceView.svelte?raw';
import type { CodexProposal, LintFinding } from '../lib/commands';

describe('MaintenanceView scroll (regression: clipped findings)', () => {
  it('.maintenance root is its own scroll container', () => {
    const block = maintenanceSource.match(/\.maintenance\s*\{[^}]*\}/)?.[0] ?? '';
    expect(block).toMatch(/overflow-y:\s*auto/);
    expect(block).toMatch(/height:\s*100%/);
  });
});

vi.mock('../lib/commands', () => ({
  getProposals: vi.fn().mockResolvedValue([]),
  acceptProposal: vi.fn().mockResolvedValue(undefined),
  rejectProposal: vi.fn().mockResolvedValue(undefined),
  getLintFindings: vi.fn().mockResolvedValue([]),
  runLint: vi.fn().mockResolvedValue({ new_findings: 0, unresolved_total: 0 }),
  resolveLintFinding: vi.fn().mockResolvedValue(undefined),
  deleteRelation: vi.fn().mockResolvedValue(undefined),
  compileEntity: vi.fn().mockResolvedValue(true),
  confirmAliasSuggestion: vi.fn().mockResolvedValue(undefined),
  undoAutoAlias: vi.fn().mockResolvedValue(undefined),
  resolveAliasCollision: vi.fn().mockResolvedValue(undefined),
  getEntity: vi.fn(),
  getEntityRelations: vi.fn().mockResolvedValue([]),
  mergeEntities: vi.fn().mockResolvedValue(undefined),
}));

import * as commands from '../lib/commands';
const m = vi.mocked(commands);

function proposal(overrides: Partial<CodexProposal> = {}): CodexProposal {
  return {
    id: 'codex_proposal:1',
    kind: 'entity_article_update',
    target: 'npc:1',
    target_name: 'Grix the Elder',
    current_text: 'Grix is a merchant.',
    payload: {
      proposed_text: 'Grix is a merchant who now trades in relics.',
      rationale: 'The party learned Grix deals in ancient relics.',
      name: null,
      entity_kind: null,
      category: null,
    },
    origin_kind: 'session',
    status: 'pending',
    created_at: '2026-07-05T00:00:00Z',
    ...overrides,
  };
}

function finding(overrides: Partial<LintFinding> = {}): LintFinding {
  return {
    id: 'lint_finding:1',
    kind: 'broken_wikilink',
    payload: { entity: 'npc:mira', link_text: 'Ghostfell' },
    created_at: '2026-07-05T00:00:00Z',
    ...overrides,
  };
}

describe('MaintenanceView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getProposals.mockResolvedValue([]);
    m.getLintFindings.mockResolvedValue([]);
    m.runLint.mockResolvedValue({ new_findings: 0, unresolved_total: 0 });
    m.resolveLintFinding.mockResolvedValue(undefined);
    m.deleteRelation.mockResolvedValue(undefined);
    m.compileEntity.mockResolvedValue(true);
    m.confirmAliasSuggestion.mockResolvedValue(undefined);
    m.undoAutoAlias.mockResolvedValue(undefined);
    m.getEntityRelations.mockResolvedValue([]);
    m.mergeEntities.mockResolvedValue(undefined);
  });

  it('renders pending proposals with kind label, target name, rationale', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await waitFor(() => expect(screen.getByText('Grix the Elder')).toBeTruthy());
    expect(screen.getByText('Article update')).toBeTruthy();
    expect(screen.getByText('The party learned Grix deals in ancient relics.')).toBeTruthy();
  });

  it('shows current vs proposed text side-by-side', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await waitFor(() => expect(screen.getByText('Grix is a merchant.')).toBeTruthy());
    expect(screen.getByText('Grix is a merchant who now trades in relics.')).toBeTruthy();
  });

  it('invokes accept_proposal with the row id, then refetches', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await screen.findByText('Grix the Elder');
    m.getProposals.mockResolvedValue([]);
    await fireEvent.click(screen.getByLabelText('Accept proposal'));
    await waitFor(() => expect(m.acceptProposal).toHaveBeenCalledWith('codex_proposal:1'));
    await waitFor(() => expect(m.getProposals).toHaveBeenCalledTimes(2));
  });

  it('invokes reject_proposal with the row id', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await screen.findByText('Grix the Elder');
    await fireEvent.click(screen.getByLabelText('Reject proposal'));
    await waitFor(() => expect(m.rejectProposal).toHaveBeenCalledWith('codex_proposal:1'));
  });

  it('renders "No pending proposals" for the empty state', async () => {
    m.getProposals.mockResolvedValue([]);
    render(MaintenanceView, { props: {} });
    await waitFor(() => expect(screen.getByText('No pending proposals')).toBeTruthy());
  });

  it('calls onCountsChanged after resolving a proposal', async () => {
    const onCountsChanged = vi.fn();
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: { onCountsChanged } });
    await screen.findByText('Grix the Elder');
    await fireEvent.click(screen.getByLabelText('Reject proposal'));
    await waitFor(() => expect(onCountsChanged).toHaveBeenCalled());
  });

  // 6. Findings tab lists findings grouped by kind with human labels
  it('findings tab lists findings grouped by kind with human labels', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({ id: 'lint_finding:1', kind: 'broken_wikilink' }),
      finding({
        id: 'lint_finding:2',
        kind: 'duplicate_entity',
        payload: { a: 'npc:k1', b: 'npc:k2', similarity: 1.0 },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await waitFor(() => expect(screen.getByText('Broken wikilink')).toBeTruthy());
    expect(screen.getByText('Possible duplicate')).toBeTruthy();
  });

  // 7. "Check campaign" button invokes run_lint and shows the summary
  it('"Check campaign" button invokes run_lint and shows the summary', async () => {
    m.runLint.mockResolvedValue({ new_findings: 3, unresolved_total: 5 });
    render(MaintenanceView, { props: { activeCampaignId: 'campaign:1' } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Check campaign' }));
    await waitFor(() => expect(m.runLint).toHaveBeenCalledWith('campaign:1'));
    expect(screen.getByText('3 new findings · 5 open')).toBeTruthy();
  });

  // 8. broken_wikilink finding has "Open entity" and "Mark resolved"
  it('broken_wikilink finding opens the entity and can be marked resolved', async () => {
    const onOpenEntity = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:1',
        kind: 'broken_wikilink',
        payload: { entity: 'npc:mira', link_text: 'Ghostfell' },
      }),
    ]);
    render(MaintenanceView, { props: { onOpenEntity } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('Ghostfell');

    await fireEvent.click(screen.getByRole('button', { name: 'Open entity' }));
    expect(onOpenEntity).toHaveBeenCalledWith('mira', 'npc');

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Mark resolved' }));
    await waitFor(() =>
      expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:1'),
    );
  });

  // 9. stale_article finding has "Compile" then resolves
  it('stale_article finding compiles the entity then resolves the finding', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:2',
        kind: 'stale_article',
        payload: { entity: 'npc:mira', reason: 'stale or uncompiled' },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('stale or uncompiled');

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Compile' }));
    await waitFor(() => expect(m.compileEntity).toHaveBeenCalledWith('npc', 'mira'));
    await waitFor(() =>
      expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:2'),
    );
  });

  // 10. scope_violation finding has "Delete edge" then resolves
  it('scope_violation finding deletes the edge then resolves the finding', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:3',
        kind: 'scope_violation',
        payload: { edge: 'relates_to:abc123', from: 'npc:x', to: 'npc:y' },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('Scope violation');

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Delete edge' }));
    await waitFor(() => expect(m.deleteRelation).toHaveBeenCalledWith('relates_to:abc123'));
    await waitFor(() =>
      expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:3'),
    );
  });

  // 11. duplicate_entity finding has two "Open" actions (a and b)
  it('duplicate_entity finding has two open actions for a and b', async () => {
    const onOpenEntity = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:4',
        kind: 'duplicate_entity',
        payload: { a: 'npc:k1', b: 'npc:k2', similarity: 1.0 },
      }),
    ]);
    render(MaintenanceView, { props: { onOpenEntity } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('Possible duplicate');

    await fireEvent.click(screen.getByRole('button', { name: 'Open A' }));
    expect(onOpenEntity).toHaveBeenCalledWith('k1', 'npc');
    await fireEvent.click(screen.getByRole('button', { name: 'Open B' }));
    expect(onOpenEntity).toHaveBeenCalledWith('k2', 'npc');
  });

  // 12. broken_wikilink with candidates shows a "did you mean?" suggestion
  it('shows a "did you mean?" suggestion and confirms it as an alternate name', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint5',
        kind: 'broken_wikilink',
        payload: {
          entity: 'npc:mira',
          link_text: 'The Quassars',
          candidates: [{ id: 'faction:q', name: 'The Quassar Family', similarity: 0.92 }],
        },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));

    expect(await screen.findByText(/did you mean/i)).toBeInTheDocument();
    expect(screen.getByText('The Quassar Family')).toBeInTheDocument();

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: /yes/i }));

    await waitFor(() =>
      expect(m.confirmAliasSuggestion).toHaveBeenCalledWith('faction:q', 'The Quassars'),
    );
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint5'));
  });

  // 13. broken_wikilink without candidates never shows "did you mean?"
  it('does not show "did you mean?" when the finding has no candidates', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({ id: 'lint6', kind: 'broken_wikilink', payload: { entity: 'npc:mira', link_text: 'Ghostfell' } }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('Ghostfell');

    expect(screen.queryByText(/did you mean/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /yes/i })).not.toBeInTheDocument();
  });

  // 14. auto_alias findings render as a collapsed, reviewable-not-required list with Undo
  it('lists auto-linked alternate names as reviewable and undoes one on request', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint7',
        kind: 'auto_alias',
        payload: { entity: 'faction:q', alias: 'The Quassars', similarity: 0.9, source: 'npc:mira' },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));

    expect(await screen.findByText(/reviewable, not required/i)).toBeInTheDocument();
    expect(screen.getByText('The Quassars')).toBeInTheDocument();

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Undo' }));

    await waitFor(() =>
      expect(m.undoAutoAlias).toHaveBeenCalledWith('faction:q', 'The Quassars', 'lint7'),
    );
  });

  // 15. alias_collision findings show both claimants
  it('alias_collision finding shows both claimants', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint8',
        kind: 'alias_collision',
        payload: { alias: 'The Quassars', a: 'faction:q1', b: 'faction:q2' },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));

    await screen.findByText('Naming conflict');
    expect(screen.getByText('The Quassars')).toBeInTheDocument();
    expect(screen.getByText('q1')).toBeInTheDocument();
    expect(screen.getByText('q2')).toBeInTheDocument();
  });

  // 16. duplicate_entity "Merge" opens the merge dialog which calls merge_entities
  it('duplicate_entity finding opens the merge dialog and merges on confirm', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint9',
        kind: 'duplicate_entity',
        payload: { a: 'faction:a1', b: 'faction:a2', similarity: 1.0 },
      }),
    ]);
    m.getEntity.mockImplementation((id, kind) =>
      Promise.resolve({
        id,
        kind,
        campaign_id: 'camp1',
        name: id === 'a1' ? 'The Free League' : 'Free League',
        aliases: [],
        summary: null,
        notes: null,
        created_at: null,
        updated_at: null,
        date_start: null,
        date_end: null,
        is_ongoing: null,
        sequence_index: null,
        era: null,
        duration_label: null,
        session_id: null,
        player_name: null,
        character_class: null,
        character_level: null,
        status: null,
        codex_article: null,
        codex_stale: null,
        codex_compiled_at: null,
      }),
    );

    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('Possible duplicate');

    await fireEvent.click(screen.getByRole('button', { name: 'Merge' }));
    const dialog = await screen.findByRole('dialog', { name: 'Merge entities' });
    await within(dialog).findByText('The Free League');

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Merge' }));

    await waitFor(() =>
      expect(m.mergeEntities).toHaveBeenCalledWith('faction:a1', 'faction:a2', {
        summary: 'keepSurvivor',
        notes: 'keepSurvivor',
      }),
    );
  });
});

describe('MaintenanceView naming-conflict card', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getProposals.mockResolvedValue([]);
    m.getLintFindings.mockResolvedValue([]);
    m.resolveLintFinding.mockResolvedValue(undefined);
    m.resolveAliasCollision.mockResolvedValue(undefined);
  });

  function collision(overrides = {}) {
    return finding({
      id: 'lint_finding:c1',
      kind: 'alias_collision',
      payload: {
        alias: 'consortium',
        a: 'faction:a',
        b: 'faction:b',
        a_name: 'Merchant Consortium',
        b_name: 'Trade Consortium',
        a_is_name: false,
        b_is_name: false,
        ...overrides,
      },
    });
  }

  it('renders entity names, not raw record ids', async () => {
    m.getLintFindings.mockResolvedValue([collision()]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    expect(await screen.findByText('Merchant Consortium')).toBeInTheDocument();
    expect(screen.getByText('Trade Consortium')).toBeInTheDocument();
    expect(screen.queryByText('faction:a')).not.toBeInTheDocument();
  });

  it('assigns the term to one entity and strips the other', async () => {
    m.getLintFindings.mockResolvedValue([collision()]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Keep on Merchant Consortium' }),
    );
    expect(m.resolveAliasCollision).toHaveBeenCalledWith(
      'lint_finding:c1',
      'faction:a',
      'faction:b',
    );
  });

  it('hides the Keep button on the side whose term is its primary name', async () => {
    m.getLintFindings.mockResolvedValue([collision({ a_is_name: true })]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await screen.findByText('Merchant Consortium');
    expect(
      screen.queryByRole('button', { name: 'Keep on Trade Consortium' }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keep on Merchant Consortium' })).toBeInTheDocument();
  });

  it('Dismiss resolves the finding without assigning', async () => {
    m.getLintFindings.mockResolvedValue([collision()]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:c1');
    expect(m.resolveAliasCollision).not.toHaveBeenCalled();
  });
});
