import { render, screen, fireEvent, waitFor, within } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest';
import MaintenanceView from './MaintenanceView.svelte';
import maintenanceSource from './MaintenanceView.svelte?raw';
import type { CodexProposal, LintFinding } from '../lib/commands';
import { i18n } from '../lib/locale.svelte';

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

beforeAll(() => {
  if (!Element.prototype.animate) {
    Object.defineProperty(Element.prototype, 'animate', {
      configurable: true,
      value: vi.fn(
        () =>
          ({
            finished: Promise.resolve(),
            cancel: vi.fn(),
          }) as unknown as Animation,
      ),
    });
  }
});

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

  it('uses the active locale for maintenance navigation and proposal actions', async () => {
    i18n.setLocale('de');
    try {
      m.getProposals.mockResolvedValue([proposal()]);
      render(MaintenanceView, { props: {} });

      expect(await screen.findByRole('heading', { name: 'Wartung' })).toBeTruthy();
      expect(screen.getByRole('tab', { name: 'Vorschläge' })).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Vorschlag annehmen' })).toBeTruthy();
      expect(screen.getByText('Sitzung')).toBeTruthy();
    } finally {
      i18n.setLocale('en');
    }
  });

  it('localizes known proposal origins and keeps unknown origins visible', async () => {
    i18n.setLocale('de');
    try {
      m.getProposals.mockResolvedValue([
        proposal({ id: 'codex_proposal:chat', origin_kind: 'chat' }),
        proposal({ id: 'codex_proposal:manual', origin_kind: 'manual' }),
        proposal({ id: 'codex_proposal:unknown', origin_kind: 'imported' }),
      ]);
      render(MaintenanceView, { props: {} });

      expect(await screen.findByText('Chat')).toBeTruthy();
      expect(screen.getByText('manuell')).toBeTruthy();
      expect(screen.getByText('imported')).toBeTruthy();
    } finally {
      i18n.setLocale('en');
    }
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
    await waitFor(() => expect(screen.getByText('Wikilinks')).toBeTruthy());
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

  // 8. broken_wikilink finding has "Open source" and "Dismiss"
  it('broken_wikilink finding opens the source and can be dismissed', async () => {
    const onOpenEntity = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:1',
        kind: 'broken_wikilink',
        payload: { entity: 'npc:mira', entity_name: 'Mira', link_text: 'Ghostfell' },
      }),
    ]);
    render(MaintenanceView, { props: { onOpenEntity } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('[[Ghostfell]] in Mira');

    await fireEvent.click(screen.getByRole('button', { name: 'Open source' }));
    expect(onOpenEntity).toHaveBeenCalledWith('mira', 'npc');

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:1'));
  });

  // 9. stale_article finding names the entity, has "Compile" then resolves
  it('stale_article finding names the entity, compiles it, then resolves', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:2',
        kind: 'stale_article',
        payload: { entity: 'npc:mira', entity_name: 'Mira', reason: 'stale or uncompiled' },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    // The card names which article is stale, not just the bare reason.
    await screen.findByText('Mira');
    expect(screen.getByText(/stale or uncompiled/)).toBeInTheDocument();

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Compile' }));
    await waitFor(() => expect(m.compileEntity).toHaveBeenCalledWith('npc', 'mira'));
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:2'));
  });

  // 9b. Compile shows a working indicator while the (slow) compile runs
  it('shows a spinner and "Compiling…" while the article compiles', async () => {
    let finishCompile: (v: boolean) => void = () => {};
    m.compileEntity.mockReturnValue(
      new Promise<boolean>((resolve) => {
        finishCompile = resolve;
      }),
    );
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint_finding:2',
        kind: 'stale_article',
        payload: { entity: 'npc:mira', entity_name: 'Mira', reason: 'stale or uncompiled' },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));

    await fireEvent.click(await screen.findByRole('button', { name: 'Compile' }));

    // While the promise is pending, the action row is replaced by a styled status
    // indicator instead of rendering the spinner inside a disabled WebKit button.
    const compiling = await screen.findByRole('status');
    expect(compiling).toHaveTextContent(/Compiling/);
    expect(compiling.querySelector('.spinner')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Compile' })).not.toBeInTheDocument();

    // Let it finish so the finding resolves and the indicator clears.
    m.getLintFindings.mockResolvedValue([]);
    finishCompile(true);
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:2'));
  });

  it('styles the compiling status spinner outside the finding actions row', () => {
    expect(maintenanceSource).toContain('.compiling-status .spinner');
    expect(maintenanceSource).toMatch(/\.compiling-status\s*\{[^}]*display:\s*inline-flex/);
    expect(maintenanceSource).toMatch(/animation-duration:\s*2s\s*!important/);
    expect(maintenanceSource).toMatch(/animation-iteration-count:\s*infinite\s*!important/);
  });

  it('animates resolved rows out even when reduced motion is enabled', () => {
    expect(maintenanceSource).toContain('out:cardOutro');
    expect(maintenanceSource).toContain('fade(');
    expect(maintenanceSource).toContain('slide(');
    expect(maintenanceSource).toMatch(
      /\.motion-list-card\s*\{[^}]*animation-duration:\s*280ms\s*!important/,
    );
    expect(maintenanceSource).toMatch(
      /\.motion-list-card\s*\{[^}]*animation-iteration-count:\s*1\s*!important/,
    );
  });

  it('keeps the findings list visible while refreshing after a resolved row', async () => {
    let finishRefresh: (value: LintFinding[]) => void = () => {};
    m.getLintFindings
      .mockResolvedValueOnce([
        finding({
          id: 'lint_finding:1',
          kind: 'broken_wikilink',
          payload: { entity: 'npc:mira', link_text: 'Ghostfell' },
        }),
        finding({
          id: 'lint_finding:2',
          kind: 'broken_wikilink',
          payload: { entity: 'npc:mira', link_text: 'Skybridge' },
        }),
      ])
      .mockReturnValueOnce(
        new Promise<LintFinding[]>((resolve) => {
          finishRefresh = resolve;
        }),
      );
    const onCountsChanged = vi.fn();
    render(MaintenanceView, { props: { onCountsChanged } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('[[Ghostfell]] in mira');

    await fireEvent.click(screen.getAllByRole('button', { name: 'Dismiss' })[0]);
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:1'));

    expect(screen.queryByText('Loading…')).not.toBeInTheDocument();
    expect(screen.getByText('[[Ghostfell]] in mira')).toBeInTheDocument();
    expect(screen.getByText('[[Skybridge]] in mira')).toBeInTheDocument();

    finishRefresh([
      finding({
        id: 'lint_finding:2',
        kind: 'broken_wikilink',
        payload: { entity: 'npc:mira', link_text: 'Skybridge' },
      }),
    ]);
    await waitFor(() => expect(onCountsChanged).toHaveBeenCalled());
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
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:3'));
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

  it('renders candidate-backed wikilinks as possible name mismatches', async () => {
    const onCreateMissingArticle = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint5',
        kind: 'broken_wikilink',
        payload: {
          entity: 'npc:mira',
          entity_name: 'Mira',
          link_text: 'The Quassars',
          candidates: [{ id: 'faction:q', name: 'The Quassar Family', similarity: 0.92 }],
        },
      }),
    ]);
    render(MaintenanceView, { props: { onCreateMissingArticle } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));

    expect(await screen.findByText('Possible name mismatch')).toBeInTheDocument();
    expect(screen.getByText('[[The Quassars]] in Mira')).toBeInTheDocument();
    expect(screen.getByText('The Quassar Family')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Use suggestion' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Create article' }));
    expect(onCreateMissingArticle).toHaveBeenCalledWith('The Quassars', 'lint5');
  });

  it('confirms a candidate-backed wikilink as an alternate name', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint5',
        kind: 'broken_wikilink',
        payload: {
          entity: 'npc:mira',
          entity_name: 'Mira',
          link_text: 'The Quassars',
          candidates: [{ id: 'faction:q', name: 'The Quassar Family', similarity: 0.92 }],
        },
      }),
    ]);
    render(MaintenanceView, { props: {} });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));
    await screen.findByText('Possible name mismatch');

    m.getLintFindings.mockResolvedValue([]);
    await fireEvent.click(screen.getByRole('button', { name: 'Use suggestion' }));

    await waitFor(() =>
      expect(m.confirmAliasSuggestion).toHaveBeenCalledWith('faction:q', 'The Quassars'),
    );
    await waitFor(() => expect(m.resolveLintFinding).toHaveBeenCalledWith('lint5'));
  });

  it('renders no-candidate wikilinks as missing articles', async () => {
    const onCreateMissingArticle = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint6',
        kind: 'broken_wikilink',
        payload: {
          entity: 'npc:mira',
          entity_name: 'Mira',
          link_text: 'Ashen Ferry',
          candidates: [],
        },
      }),
    ]);
    render(MaintenanceView, { props: { onCreateMissingArticle } });
    await fireEvent.click(screen.getByRole('tab', { name: 'Findings' }));

    expect(await screen.findByText('Missing article')).toBeInTheDocument();
    expect(screen.getByText('[[Ashen Ferry]] in Mira')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Use suggestion' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Create article' }));
    expect(onCreateMissingArticle).toHaveBeenCalledWith('Ashen Ferry', 'lint6');
  });

  // 14. auto_alias findings render as a collapsed, reviewable-not-required list with Undo
  it('lists auto-linked alternate names as reviewable and undoes one on request', async () => {
    m.getLintFindings.mockResolvedValue([
      finding({
        id: 'lint7',
        kind: 'auto_alias',
        payload: {
          entity: 'faction:q',
          alias: 'The Quassars',
          similarity: 0.9,
          source: 'npc:mira',
        },
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

  it('hides both Keep buttons when a party is deleted (no enriched name)', async () => {
    // b_name absent → entity B was soft-deleted; enrichment couldn't resolve it.
    m.getLintFindings.mockResolvedValue([collision({ b_name: undefined, b_is_name: undefined })]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await screen.findByText('Merchant Consortium');
    expect(screen.queryByRole('button', { name: /^Keep on/ })).not.toBeInTheDocument();
    // Dismiss + Merge still available.
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Merge…' })).toBeInTheDocument();
  });
});
