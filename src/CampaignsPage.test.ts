import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import CampaignsPage from './CampaignsPage.svelte';
import * as commands from './lib/commands';
import type { Source } from './lib/commands';

vi.mock('./lib/commands', () => ({
  getCollections: vi.fn().mockResolvedValue([]),
  getCampaigns: vi.fn().mockResolvedValue([]),
  createCollection: vi.fn(),
  updateCollection: vi.fn(),
  deleteCollection: vi.fn(),
  getCampaignCollections: vi.fn().mockResolvedValue([]),
  addCampaignCollection: vi.fn(),
  removeCampaignCollection: vi.fn(),
  createCampaign: vi.fn(),
  deleteCampaign: vi.fn(),
  getSources: vi.fn().mockResolvedValue([]),
  deleteSource: vi.fn(),
}));

const mockedCommands = vi.mocked(commands);

function makeCollection(id: string, name: string) {
  return { id, name, description: null };
}

function makeCampaign(id: string, name: string) {
  return { id, name, system: null };
}

describe('CampaignsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedCommands.getCollections.mockResolvedValue([]);
    mockedCommands.getCampaigns.mockResolvedValue([]);
    mockedCommands.getSources.mockResolvedValue([]);
    mockedCommands.getCampaignCollections.mockResolvedValue([]);
  });

  // ── Fix 1: commitRename closes input on error ──────────────────────

  describe('commitRename — renamingId reset on error', () => {
    it('resets renamingId after a successful rename', async () => {
      const col = makeCollection('col-1', 'Old Name');
      mockedCommands.getCollections.mockResolvedValue([col]);
      mockedCommands.updateCollection.mockResolvedValue({ ...col, name: 'New Name' });

      render(CampaignsPage);

      // Wait for initial load to populate the collection list
      const renameBtn = await screen.findByTitle('Rename');
      await fireEvent.click(renameBtn);

      // The rename input should be visible
      const renameInput = await screen.findByDisplayValue('Old Name');
      await fireEvent.input(renameInput, { target: { value: 'New Name' } });
      await fireEvent.blur(renameInput);

      await waitFor(() => {
        expect(screen.queryByDisplayValue('New Name')).toBeNull();
      });
    });

    it('resets renamingId when updateCollection throws', async () => {
      const col = makeCollection('col-1', 'Old Name');
      mockedCommands.getCollections.mockResolvedValue([col]);
      mockedCommands.updateCollection.mockRejectedValue(new Error('DB error'));

      render(CampaignsPage);

      const renameBtn = await screen.findByTitle('Rename');
      await fireEvent.click(renameBtn);

      const renameInput = await screen.findByDisplayValue('Old Name');
      await fireEvent.input(renameInput, { target: { value: 'Something New' } });
      await fireEvent.blur(renameInput);

      // The rename input should be gone even after an error (renamingId = null in catch)
      await waitFor(() => {
        expect(screen.queryByDisplayValue('Something New')).toBeNull();
      });

      // The error message should be shown
      await waitFor(() => {
        expect(screen.getByText('Error: DB error')).toBeTruthy();
      });
    });
  });

  // ── Fix 2: Stale response discarded when selection changes during load ─

  describe('selectCollection — stale response is discarded', () => {
    it('does not apply results from a superseded collection load', async () => {
      const col1 = makeCollection('col-1', 'Alpha');
      const col2 = makeCollection('col-2', 'Beta');
      mockedCommands.getCollections.mockResolvedValue([col1, col2]);

      // col-1 resolves slowly; col-2 resolves immediately
      let resolveCol1: (v: Source[]) => void;
      const col1Promise = new Promise<Source[]>((res) => {
        resolveCol1 = res;
      });
      const col2Sources = [
        {
          id: 's-2',
          filename: 'beta.pdf',
          display_name: 'Beta Source',
          source_type: 'pdf',
          page_count: 10,
          index_status: 'done',
          embed_model: 'text-embed-3',
          collection_id: 'col-2',
        },
      ];
      mockedCommands.getSources
        .mockImplementationOnce(() => col1Promise)
        .mockResolvedValueOnce(col2Sources);

      render(CampaignsPage);

      // Click col-1 first (slow), then immediately click col-2 (fast)
      const [btnAlpha, btnBeta] = await screen.findAllByRole('button', {
        name: /Alpha|Beta/,
      });
      await fireEvent.click(btnAlpha);
      await fireEvent.click(btnBeta);

      // Let col-2 settle
      await waitFor(() => {
        expect(screen.getByText('Beta Source')).toBeTruthy();
      });

      // Now resolve the stale col-1 promise — should not overwrite col-2 results
      resolveCol1!([
        {
          id: 's-1',
          filename: 'alpha.pdf',
          display_name: 'Alpha Source',
          source_type: 'pdf',
          page_count: 5,
          index_status: 'done',
          embed_model: 'text-embed-3',
          collection_id: 'col-1',
        },
      ]);

      // Alpha Source must NOT appear — the stale response was discarded
      await waitFor(() => {
        expect(screen.queryByText('Alpha Source')).toBeNull();
      });

      // Beta Source remains
      expect(screen.getByText('Beta Source')).toBeTruthy();
    });
  });

  // ── Fix 3: unsubscribedCollections excludes already-subscribed collections ─

  describe('unsubscribedCollections derived state', () => {
    it('excludes collections already subscribed to the selected campaign', async () => {
      const col1 = makeCollection('col-1', 'Rulebook');
      const col2 = makeCollection('col-2', 'Bestiary');
      const campaign = makeCampaign('camp-1', 'Dragon Campaign');

      mockedCommands.getCollections.mockResolvedValue([col1, col2]);
      mockedCommands.getCampaigns.mockResolvedValue([campaign]);
      // col-1 is already subscribed; col-2 is not
      mockedCommands.getCampaignCollections.mockResolvedValue([col1]);

      render(CampaignsPage);

      // Select the campaign
      const campaignBtn = await screen.findByRole('button', { name: /Dragon Campaign/ });
      await fireEvent.click(campaignBtn);

      // Wait for campaign collections to load
      await waitFor(() => {
        // col-1 should appear as a subscribed chip (with unsubscribe button)
        expect(screen.getByTitle('Unsubscribe')).toBeTruthy();
      });

      // col-2 should appear in the "Add collection" row (unsubscribed)
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /\+ Bestiary/ })).toBeTruthy();
      });

      // col-1 should NOT appear in the add row (already subscribed)
      const addButtons = screen.queryAllByRole('button', { name: /\+ Rulebook/ });
      expect(addButtons).toHaveLength(0);
    });

    it('shows all collections as addable when campaign has no subscriptions', async () => {
      const col1 = makeCollection('col-1', 'Rulebook');
      const col2 = makeCollection('col-2', 'Bestiary');
      const campaign = makeCampaign('camp-1', 'Empty Campaign');

      mockedCommands.getCollections.mockResolvedValue([col1, col2]);
      mockedCommands.getCampaigns.mockResolvedValue([campaign]);
      mockedCommands.getCampaignCollections.mockResolvedValue([]);

      render(CampaignsPage);

      const campaignBtn = await screen.findByRole('button', { name: /Empty Campaign/ });
      await fireEvent.click(campaignBtn);

      // Both collections should appear as addable
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /\+ Rulebook/ })).toBeTruthy();
        expect(screen.getByRole('button', { name: /\+ Bestiary/ })).toBeTruthy();
      });
    });
  });
});
