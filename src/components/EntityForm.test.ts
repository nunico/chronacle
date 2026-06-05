import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import EntityForm from './EntityForm.svelte';
import type { EntityKind, GraphNode, Session } from '../lib/commands';

const mockNode = (overrides: Partial<GraphNode> = {}): GraphNode => ({
  id: 'abc',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  summary: null,
  notes: null,
  created_at: null,
  updated_at: null,
  date_start: null, date_end: null, is_ongoing: null,
  sequence_index: null, era: null, duration_label: null,
  session_id: null,
  player_name: null, character_class: null,
  character_level: null, status: null,
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
  it('renders name field for any entity kind', () => {
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null } });
    expect(screen.getByLabelText(/name/i)).toBeTruthy();
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
    expect((screen.getByLabelText(/name/i) as HTMLInputElement).value).toBe('Vex');
  });

  it('emits save event with input on submit', async () => {
    const onSave = vi.fn();
    render(EntityForm, {
      props: { kind: 'npc' as EntityKind, node: null, onsave: onSave },
    });
    await fireEvent.input(screen.getByLabelText(/name/i), { target: { value: 'New NPC' } });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].name).toBe('New NPC');
    expect(onSave.mock.calls[0][0].summary).toBeNull();
    expect(onSave.mock.calls[0][0].playerName).toBeNull();
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
    const sessions = [mockSession(), mockSession({ id: 'sess2', session_number: 2, title: 'The Dungeon' })];
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
    render(EntityForm, { props: { kind: 'event' as EntityKind, node: null, sessions, onsave: onSave } });
    await fireEvent.input(screen.getByLabelText(/name/i), { target: { value: 'Battle of Helm' } });
    const select = screen.getByLabelText(/session/i) as HTMLSelectElement;
    await fireEvent.change(select, { target: { value: 'sess1' } });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].sessionId).toBe('sess1');
  });

  it('sets sessionId null when no session selected', async () => {
    const onSave = vi.fn();
    render(EntityForm, { props: { kind: 'event' as EntityKind, node: null, onsave: onSave } });
    await fireEvent.input(screen.getByLabelText(/name/i), { target: { value: 'Battle' } });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].sessionId).toBeNull();
  });
});
