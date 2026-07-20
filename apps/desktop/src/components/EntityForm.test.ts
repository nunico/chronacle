import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import EntityForm from './EntityForm.svelte';
import type { EntityKind, GraphNode, Session, RelatedEntity } from '../lib/commands';

// Module-level mock: getEntityRelations/listVaultConflicts default to [] so
// existing tests are unaffected; individual tests override as needed.
vi.mock('../lib/commands', () => ({
  getEntityRelations: vi.fn().mockResolvedValue([]),
  listVaultConflicts: vi.fn().mockResolvedValue([]),
}));

import * as commands from '../lib/commands';
import { i18n } from '../lib/locale.svelte';

const mockNode = (overrides: Partial<GraphNode> = {}): GraphNode => ({
  id: 'abc',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
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
  ...overrides,
});

const mockSession = (overrides: Partial<Session> = {}): Session => ({
  id: 'sess1',
  campaign_id: 'camp1',
  session_number: 1,
  title: 'The Beginning',
  date_played: '2024-01-01',
  notes: '',
  created_at: null,
  updated_at: null,
  ...overrides,
});

describe('EntityForm', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(commands.getEntityRelations).mockResolvedValue([]);
    vi.mocked(commands.listVaultConflicts).mockResolvedValue([]);
  });

  it('renders name field for any entity kind', () => {
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null } });
    expect(screen.getByLabelText('Name', { exact: true })).toBeTruthy();
  });

  it('shows temporal fields for event kind', () => {
    render(EntityForm, { props: { kind: 'event' as EntityKind, node: null } });
    expect(screen.getByLabelText(/date start/i)).toBeTruthy();
    expect(screen.getByLabelText(/sequence index/i)).toBeTruthy();
  });

  it('does NOT show temporal fields for npc kind', () => {
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null } });
    expect(screen.queryByLabelText(/date start/i)).toBeNull();
  });

  it('shows player fields for player_character kind', () => {
    render(EntityForm, { props: { kind: 'player_character' as EntityKind, node: null } });
    expect(screen.getByLabelText(/player name/i)).toBeTruthy();
    expect(screen.getByLabelText(/character class/i)).toBeTruthy();
    expect(screen.getByLabelText(/character level/i)).toBeTruthy();
  });

  it('pre-fills fields when editing an existing node', () => {
    const node = mockNode({ name: 'Vex', kind: 'npc' });
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });
    expect((screen.getByLabelText('Name', { exact: true }) as HTMLInputElement).value).toBe('Vex');
  });

  it('emits save event with input on submit', async () => {
    const onSave = vi.fn();
    render(EntityForm, {
      props: { kind: 'npc' as EntityKind, node: null, onsave: onSave },
    });
    await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
      target: { value: 'New NPC' },
    });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].name).toBe('New NPC');
    expect(onSave.mock.calls[0][0].summary).toBeNull();
    expect(onSave.mock.calls[0][0].playerName).toBeNull();
  });

  // AliasField -> EntityForm submit must always carry the COMPLETE alternate-name
  // array, never `null`/`undefined` — an omitted `aliases` field means "preserve"
  // on the backend, so a partial edit would otherwise silently no-op.
  it('always submits the complete alternate-name array, never null', async () => {
    const onSave = vi.fn();
    const node = mockNode({ aliases: ['The Quassars'] });
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node, onsave: onSave } });

    await fireEvent.input(screen.getByPlaceholderText('Add an alternate name'), {
      target: { value: 'Quassar Clan' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    await fireEvent.submit(screen.getByRole('form'));

    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].aliases).toEqual(['The Quassars', 'Quassar Clan']);
  });

  it('submits an empty alternate-name array (not null) for a new entity with none added', async () => {
    const onSave = vi.fn();
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null, onsave: onSave } });

    await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
      target: { value: 'New NPC' },
    });
    await fireEvent.submit(screen.getByRole('form'));

    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].aliases).toEqual([]);
    expect(onSave.mock.calls[0][0].aliases).not.toBeNull();
  });

  it('shows prop-supplied field error for the name field', () => {
    render(EntityForm, {
      props: {
        kind: 'npc' as EntityKind,
        node: null,
        error: { code: 'VALIDATION', message: 'Too long', field: 'name' },
      },
    });
    expect(screen.getByText(/too long/i)).toBeTruthy();
  });

  it('shows inline validation error when name is empty on submit', async () => {
    render(EntityForm, {
      props: { kind: 'npc' as EntityKind, node: null, onsave: vi.fn() },
    });
    await fireEvent.submit(screen.getByRole('form'));
    expect(screen.getByText(/name is required/i)).toBeTruthy();
  });

  it('shows session dropdown for event kind', () => {
    const sessions = [
      mockSession(),
      mockSession({ id: 'sess2', session_number: 2, title: 'The Dungeon' }),
    ];
    render(EntityForm, { props: { kind: 'event' as EntityKind, node: null, sessions } });
    expect(screen.getByLabelText(/session/i)).toBeTruthy();
    expect(screen.getByText(/#1: The Beginning/i)).toBeTruthy();
    expect(screen.getByText(/#2: The Dungeon/i)).toBeTruthy();
  });

  it('does not show session dropdown for npc kind', () => {
    const sessions = [mockSession()];
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null, sessions } });
    expect(screen.queryByLabelText(/session/i)).toBeNull();
  });

  it('includes sessionId in save payload for event kind', async () => {
    const onSave = vi.fn();
    const sessions = [mockSession()];
    render(EntityForm, {
      props: { kind: 'event' as EntityKind, node: null, sessions, onsave: onSave },
    });
    await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
      target: { value: 'Battle of Helm' },
    });
    const select = screen.getByLabelText(/session/i) as HTMLSelectElement;
    await fireEvent.change(select, { target: { value: 'sess1' } });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].sessionId).toBe('sess1');
  });

  it('sets sessionId null when no session selected', async () => {
    const onSave = vi.fn();
    render(EntityForm, { props: { kind: 'event' as EntityKind, node: null, onsave: onSave } });
    await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
      target: { value: 'Battle' },
    });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].sessionId).toBeNull();
  });

  // ── Relationships section ──────────────────────────────────────────────────

  it('does not show the Relationships section when no node is set (create form)', () => {
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null } });
    expect(screen.queryByText(/relationships/i)).toBeNull();
    expect(commands.getEntityRelations).not.toHaveBeenCalled();
  });

  it('renders both outbound and inbound relations with names and rel_type visible', async () => {
    const relations: RelatedEntity[] = [
      {
        id: 'loc1',
        kind: 'location',
        name: 'Shadowhaven',
        rel_type: 'lives_in',
        direction: 'outbound',
      },
      {
        id: 'fac1',
        kind: 'faction',
        name: 'Shadow Guild',
        rel_type: 'member_of',
        direction: 'inbound',
      },
    ];
    vi.mocked(commands.getEntityRelations).mockResolvedValue(relations);
    const node = mockNode();
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });

    await waitFor(() => {
      expect(screen.getByText('Shadowhaven')).toBeTruthy();
    });
    expect(screen.getByText('Shadow Guild')).toBeTruthy();
    expect(screen.getByText('lives_in')).toBeTruthy();
    expect(screen.getByText('member_of')).toBeTruthy();
    // Direction distinguishable: outbound → and inbound ←
    const arrows = screen.getAllByText(/^[→←]$/);
    expect(arrows.length).toBe(2);
    const directions = arrows.map((el) => el.textContent);
    expect(directions).toContain('→');
    expect(directions).toContain('←');
  });

  it('localizes relationship kind labels with the active display language', async () => {
    vi.mocked(commands.getEntityRelations).mockResolvedValue([
      {
        id: 'loc1',
        kind: 'location',
        name: 'Shadowhaven',
        rel_type: 'lives_in',
        direction: 'outbound',
      },
    ]);
    i18n.setLocale('de');
    try {
      render(EntityForm, { props: { kind: 'npc' as EntityKind, node: mockNode() } });
      expect(await screen.findByText('Ort')).toBeTruthy();
    } finally {
      i18n.setLocale('en');
    }
  });

  it('shows the empty state when getEntityRelations returns []', async () => {
    vi.mocked(commands.getEntityRelations).mockResolvedValue([]);
    const node = mockNode();
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /relationships/i })).toBeTruthy();
    });
    expect(screen.getByText(/no relationships yet/i)).toBeTruthy();
  });

  it('calls onOpenEntity with the related entity id and kind when a row is clicked', async () => {
    const relations: RelatedEntity[] = [
      {
        id: 'loc1',
        kind: 'location',
        name: 'Shadowhaven',
        rel_type: 'lives_in',
        direction: 'outbound',
      },
    ];
    vi.mocked(commands.getEntityRelations).mockResolvedValue(relations);
    const onOpenEntity = vi.fn();
    const node = mockNode();
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node, onOpenEntity } });

    await waitFor(() => {
      expect(screen.getByText('Shadowhaven')).toBeTruthy();
    });
    // When onOpenEntity is provided, a button wraps each row
    const btn = screen.getByRole('button', { name: /open shadowhaven/i });
    await fireEvent.click(btn);
    expect(onOpenEntity).toHaveBeenCalledOnce();
    expect(onOpenEntity).toHaveBeenCalledWith('loc1', 'location');
  });

  it('rows are non-interactive (no button) when onOpenEntity is not provided', async () => {
    const relations: RelatedEntity[] = [
      {
        id: 'loc1',
        kind: 'location',
        name: 'Shadowhaven',
        rel_type: 'lives_in',
        direction: 'outbound',
      },
    ];
    vi.mocked(commands.getEntityRelations).mockResolvedValue(relations);
    const node = mockNode();
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });

    await waitFor(() => {
      expect(screen.getByText('Shadowhaven')).toBeTruthy();
    });
    // Without onOpenEntity, no button wraps the row content
    expect(screen.queryByRole('button', { name: /open shadowhaven/i })).toBeNull();
  });

  // ── Vault conflict banner ──────────────────────────────────────────────────

  it('shows a conflict banner when the open entity is conflicted', async () => {
    vi.mocked(commands.listVaultConflicts).mockResolvedValue([
      { id: 'n1', kind: 'npc', name: 'Seraphina', key: 'k.md', sidecarKey: 'k.conflict.md' },
    ]);
    const node = mockNode({ id: 'n1', kind: 'npc' });
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });
    expect(await screen.findByText(/unsynced vault edits in conflict/i)).toBeInTheDocument();
  });

  it('does not show a conflict banner for a different entity', async () => {
    vi.mocked(commands.listVaultConflicts).mockResolvedValue([
      { id: 'other', kind: 'npc', name: 'Someone Else', key: 'k.md', sidecarKey: 'k.conflict.md' },
    ]);
    const node = mockNode({ id: 'n1', kind: 'npc' });
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });
    await waitFor(() => expect(commands.listVaultConflicts).toHaveBeenCalled());
    expect(screen.queryByText(/unsynced vault edits in conflict/i)).toBeNull();
  });
});
