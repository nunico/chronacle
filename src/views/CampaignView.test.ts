import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import CampaignView from './CampaignView.svelte';
import * as commands from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getCollections: vi.fn().mockResolvedValue([]),
  getCampaignCollections: vi.fn().mockResolvedValue([]),
  addCampaignCollection: vi.fn(),
  removeCampaignCollection: vi.fn(),
  getSources: vi.fn().mockResolvedValue([]),
  deleteSource: vi.fn(),
  createCampaign: vi.fn(),
  updateCampaign: vi.fn(),
  deleteCampaign: vi.fn(),
}));

const m = vi.mocked(commands);

function col(id: string, name: string) {
  return { id, name, description: null };
}
function camp(id: string, name: string, system: string | null = null) {
  return { id, name, system };
}
function src(id: string, name: string, status = 'done') {
  return {
    id,
    filename: name,
    display_name: name,
    source_type: 'rules',
    page_count: 12,
    index_status: status,
    embed_model: 'nomic-embed-text-v1.5',
    collection_id: null,
  };
}

describe('CampaignView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getCollections.mockResolvedValue([]);
    m.getCampaignCollections.mockResolvedValue([]);
    m.getSources.mockResolvedValue([]);
  });

  it('renders the active campaign name in the hero', async () => {
    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Hollow Reach', '5e')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Hollow Reach/i })).toBeTruthy();
    });
  });

  it('shows a Global hero when no campaign is active', async () => {
    render(CampaignView, {
      props: {
        activeCampaignId: null,
        campaigns: [],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Global/i })).toBeTruthy();
    });
  });

  it('toggles subscription via the switch and calls addCampaignCollection', async () => {
    m.getCollections.mockResolvedValue([col('c-1', 'Rules')]);
    m.getCampaignCollections.mockResolvedValue([]);

    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Reach')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });

    const sw = await screen.findByRole('switch', { name: /Subscribe to Rules/i });
    await fireEvent.click(sw);

    await waitFor(() => {
      expect(m.addCampaignCollection).toHaveBeenCalledWith('camp-1', 'c-1');
    });
  });

  it('expands a collection and calls onOpenUpload(collectionId) on Add book', async () => {
    m.getCollections.mockResolvedValue([col('c-1', 'Rules')]);
    m.getCampaignCollections.mockResolvedValue([col('c-1', 'Rules')]);
    m.getSources.mockResolvedValue([src('s-1', 'PHB.pdf')]);

    const onOpenUpload = vi.fn();
    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Reach')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload,
        refreshCampaigns: vi.fn(),
      },
    });

    // Click the collection header to expand
    const head = await screen.findByRole('button', { name: /Rules/ });
    await fireEvent.click(head);

    // Sources are listed, and Add book is reachable
    await waitFor(() => {
      expect(screen.getByText('PHB.pdf')).toBeTruthy();
    });
    const addBtn = screen.getByRole('button', { name: /Add book/i });
    await fireEvent.click(addBtn);
    expect(onOpenUpload).toHaveBeenCalledWith('c-1');
  });

  it('creates a new campaign and sets it active', async () => {
    const created = camp('new-1', 'New Saga', '5e');
    m.createCampaign.mockResolvedValue(created);
    const setActive = vi.fn();
    const refresh = vi.fn().mockResolvedValue(undefined);

    render(CampaignView, {
      props: {
        activeCampaignId: null,
        campaigns: [],
        setActiveCampaignId: setActive,
        onOpenUpload: vi.fn(),
        refreshCampaigns: refresh,
      },
    });

    // Open Manage campaigns
    const manageHead = await screen.findByRole('button', { name: /Manage campaigns/i });
    await fireEvent.click(manageHead);

    const nameInput = await screen.findByPlaceholderText('New campaign name');
    await fireEvent.input(nameInput, { target: { value: 'New Saga' } });
    const sysInput = screen.getByPlaceholderText('System (optional)');
    await fireEvent.input(sysInput, { target: { value: '5e' } });

    const createBtn = screen.getByRole('button', { name: /\+ Create/ });
    await fireEvent.click(createBtn);

    await waitFor(() => {
      expect(m.createCampaign).toHaveBeenCalledWith('New Saga', '5e');
    });
    await waitFor(() => {
      expect(refresh).toHaveBeenCalled();
      expect(setActive).toHaveBeenCalledWith('new-1');
    });
  });
});
