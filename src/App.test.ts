import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import App from './App.svelte';
import * as commands from './lib/commands';

// ── Mock @tauri-apps/plugin-dialog ────────────────────────────────────────
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

// ── Mock @tauri-apps/api/core ─────────────────────────────────────────────
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// ── Mock @tauri-apps/api/event ────────────────────────────────────────────
const mockListeners: Record<string, Array<(payload: unknown) => void>> = {};
const unlistenFn = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (event: { payload: unknown }) => void) => {
    if (!mockListeners[event]) mockListeners[event] = [];
    mockListeners[event].push((payload) => cb({ payload }));
    return Promise.resolve(unlistenFn);
  }),
}));

// ── Mock ./lib/commands ───────────────────────────────────────────────────
vi.mock('./lib/commands', () => ({
  getCollections: vi.fn().mockResolvedValue([]),
  createCollection: vi.fn(),
  uploadSource: vi.fn().mockResolvedValue({ id: 'source:1' }),
  getSources: vi.fn().mockResolvedValue([]),
  deleteSource: vi.fn(),
  getMruCollectionId: vi.fn().mockReturnValue(null),
  setMruCollectionId: vi.fn(),
}));

import * as dialog from '@tauri-apps/plugin-dialog';
import * as tauriCore from '@tauri-apps/api/core';

const mockedCommands = vi.mocked(commands);
const mockedDialog = vi.mocked(dialog);
const mockedInvoke = vi.mocked(tauriCore.invoke);

function makeCollection(id: string, name: string) {
  return { id, name, description: null };
}

beforeEach(() => {
  vi.clearAllMocks();
  Object.keys(mockListeners).forEach((k) => delete mockListeners[k]);
  // Default: invoke('get_chat_history') returns empty array
  mockedInvoke.mockResolvedValue([]);
  mockedCommands.getCollections.mockResolvedValue([]);
  mockedCommands.getMruCollectionId.mockReturnValue(null);
  mockedCommands.uploadSource.mockResolvedValue({ id: 'source:1' });
});

// ── Picker visibility ─────────────────────────────────────────────────────

describe('collection picker — visibility', () => {
  it('does not show picker on initial render', () => {
    render(App);
    expect(screen.queryByRole('heading', { name: /to collection/i })).toBeNull();
  });

  it('shows picker after file is selected', async () => {
    mockedDialog.open.mockResolvedValue('/home/gm/rules.pdf');
    mockedCommands.getCollections.mockResolvedValue([makeCollection('col-1', 'Rulebook')]);

    render(App);

    const uploadBtn = screen.getByRole('button', { name: /Upload PDF/i });
    await fireEvent.click(uploadBtn);

    await waitFor(() => {
      expect(screen.getByText(/to collection/i)).toBeTruthy();
    });
  });

  it('does not show picker when file dialog is cancelled', async () => {
    mockedDialog.open.mockResolvedValue(null);

    render(App);

    const uploadBtn = screen.getByRole('button', { name: /Upload PDF/i });
    await fireEvent.click(uploadBtn);

    await waitFor(() => {
      expect(screen.queryByText(/to collection/i)).toBeNull();
    });
  });
});

// ── Picker filename display ───────────────────────────────────────────────

describe('collection picker — filename display', () => {
  it('displays the filename from the selected path', async () => {
    mockedDialog.open.mockResolvedValue('/home/gm/my-rulebook.pdf');
    mockedCommands.getCollections.mockResolvedValue([makeCollection('col-1', 'Rulebook')]);

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));

    await waitFor(() => {
      expect(screen.getByText(/my-rulebook\.pdf/)).toBeTruthy();
    });
  });
});

// ── Picker cancel ─────────────────────────────────────────────────────────

describe('collection picker — cancel', () => {
  it('hides picker when Cancel is clicked', async () => {
    mockedDialog.open.mockResolvedValue('/home/gm/rules.pdf');
    mockedCommands.getCollections.mockResolvedValue([makeCollection('col-1', 'Rulebook')]);

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));

    await waitFor(() => screen.getByText(/to collection/i));

    // Use testid to unambiguously target the dialog's Cancel button
    const cancelBtn = screen.getByTestId('picker-cancel');
    await fireEvent.click(cancelBtn);

    await waitFor(() => {
      expect(screen.queryByText(/to collection/i)).toBeNull();
    });
  });
});

// ── MRU pre-selection ─────────────────────────────────────────────────────

describe('collection picker — MRU pre-selection', () => {
  it('pre-selects the MRU collection when it exists in the list', async () => {
    const col1 = makeCollection('col-1', 'Alpha');
    const col2 = makeCollection('col-2', 'Beta');
    mockedCommands.getCollections.mockResolvedValue([col1, col2]);
    mockedCommands.getMruCollectionId.mockReturnValue('col-2');
    mockedDialog.open.mockResolvedValue('/path/rules.pdf');

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));

    await waitFor(() => screen.getByText(/to collection/i));

    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('col-2');
  });

  it('falls back to the first collection when MRU is not in the list', async () => {
    const col1 = makeCollection('col-1', 'Alpha');
    mockedCommands.getCollections.mockResolvedValue([col1]);
    mockedCommands.getMruCollectionId.mockReturnValue('col-stale');
    mockedDialog.open.mockResolvedValue('/path/rules.pdf');

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));

    await waitFor(() => screen.getByText(/to collection/i));

    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('col-1');
  });
});

// ── Create new collection inline ──────────────────────────────────────────

describe('collection picker — create new collection', () => {
  it('shows name input when "+ Create new collection" is clicked', async () => {
    mockedDialog.open.mockResolvedValue('/path/rules.pdf');
    mockedCommands.getCollections.mockResolvedValue([]);

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    const newBtn = screen.getByRole('button', { name: /Create new collection/i });
    await fireEvent.click(newBtn);

    expect(screen.getByPlaceholderText('New collection name')).toBeTruthy();
  });

  it('calls createCollection and adds it to the list', async () => {
    const newCol = makeCollection('col-new', 'My New Collection');
    mockedCommands.createCollection.mockResolvedValue(newCol);
    mockedDialog.open.mockResolvedValue('/path/rules.pdf');
    mockedCommands.getCollections.mockResolvedValue([]);

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    await fireEvent.click(screen.getByRole('button', { name: /Create new collection/i }));
    const nameInput = screen.getByPlaceholderText('New collection name');
    await fireEvent.input(nameInput, { target: { value: 'My New Collection' } });
    await fireEvent.click(screen.getByRole('button', { name: /^Create$/ }));

    await waitFor(() => {
      expect(mockedCommands.createCollection).toHaveBeenCalledWith('My New Collection');
    });

    await waitFor(() => {
      // new collection name should appear in the select
      expect(screen.getByText('My New Collection')).toBeTruthy();
    });
  });

  it('shows error when createCollection throws', async () => {
    mockedCommands.createCollection.mockRejectedValue(new Error('DB error'));
    mockedDialog.open.mockResolvedValue('/path/rules.pdf');
    mockedCommands.getCollections.mockResolvedValue([]);

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    await fireEvent.click(screen.getByRole('button', { name: /Create new collection/i }));
    const nameInput = screen.getByPlaceholderText('New collection name');
    await fireEvent.input(nameInput, { target: { value: 'Bad Col' } });
    await fireEvent.click(screen.getByRole('button', { name: /^Create$/ }));

    await waitFor(() => {
      expect(screen.getByText(/DB error/)).toBeTruthy();
    });
  });
});

// ── Confirm upload ────────────────────────────────────────────────────────

describe('confirmUpload', () => {
  it('closes picker and calls uploadSource with the selected collection', async () => {
    const col = makeCollection('col-1', 'Rulebook');
    mockedCommands.getCollections.mockResolvedValue([col]);
    mockedDialog.open.mockResolvedValue('/home/gm/rules.pdf');

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    const confirmBtn = screen.getByRole('button', { name: /^Upload$/ });
    await fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(screen.queryByText(/to collection/i)).toBeNull();
    });

    await waitFor(() => {
      expect(mockedCommands.uploadSource).toHaveBeenCalledWith(
        '/home/gm/rules.pdf',
        'rules.pdf',
        'rules',
        'col-1',
      );
    });
  });

  it('saves the chosen collection as the MRU', async () => {
    const col = makeCollection('col-2', 'Bestiary');
    mockedCommands.getCollections.mockResolvedValue([col]);
    mockedDialog.open.mockResolvedValue('/path/bestiary.pdf');

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    await fireEvent.click(screen.getByRole('button', { name: /^Upload$/ }));

    await waitFor(() => {
      expect(mockedCommands.setMruCollectionId).toHaveBeenCalledWith('col-2');
    });
  });

  it('shows upload status banner while uploading', async () => {
    const col = makeCollection('col-1', 'Rulebook');
    mockedCommands.getCollections.mockResolvedValue([col]);
    mockedDialog.open.mockResolvedValue('/home/gm/rules.pdf');
    // Delay uploadSource so we can observe the uploading state
    mockedCommands.uploadSource.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve({ id: 'source:1' }), 100)),
    );

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    await fireEvent.click(screen.getByRole('button', { name: /^Upload$/ }));

    // Status banner should appear immediately
    await waitFor(() => {
      expect(screen.getByText('rules.pdf')).toBeTruthy();
    });
  });

  it('Upload button is disabled when no collection is selected', async () => {
    mockedDialog.open.mockResolvedValue('/path/rules.pdf');
    mockedCommands.getCollections.mockResolvedValue([]);

    render(App);
    await fireEvent.click(screen.getByRole('button', { name: /Upload PDF/i }));
    await waitFor(() => screen.getByText(/to collection/i));

    const confirmBtn = screen.getByRole('button', { name: /^Upload$/ });
    expect((confirmBtn as HTMLButtonElement).disabled).toBe(true);
  });
});
