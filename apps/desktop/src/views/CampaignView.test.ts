import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/svelte';
import CampaignView from './CampaignView.svelte';
import * as commands from '../lib/commands';
import type { Campaign } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import {
  entityScope,
  newEntityScope,
  newSessionScope,
  oracleScope,
  ruleScope,
  sessionScope,
} from '../lib/drafts/draft-state';
import { rememberedRuleRecovery, rememberRuleRecovery } from '../lib/drafts/rule-note-presentation';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

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
  getCodexStatus: vi.fn(),
  compileCollection: vi.fn(),
  cancelCompile: vi.fn(),
}));

const m = vi.mocked(commands);

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

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

function renderView(
  overrides: Partial<{
    activeCampaignId: string | null;
    campaigns: Campaign[];
    setActiveCampaignId: (id: string | null) => void;
    onOpenUpload: (collectionId: string) => void;
    refreshCampaigns: () => Promise<void>;
    draftCoordinator: DraftCoordinator;
  }> = {},
) {
  return render(CampaignView, {
    props: {
      activeCampaignId: 'camp-1',
      campaigns: [camp('camp-1', 'Reach')],
      setActiveCampaignId: vi.fn(),
      onOpenUpload: vi.fn(),
      refreshCampaigns: vi.fn(),
      ...overrides,
    },
  });
}

describe('CampaignView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getCollections.mockResolvedValue([]);
    m.getCampaignCollections.mockResolvedValue([]);
    m.getSources.mockResolvedValue([]);
    m.getCodexStatus.mockResolvedValue({
      stale_entities: 0,
      total_entities: 0,
      rules_stale: 0,
      rule_entries: 0,
    });
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

  it('shows an empty-state hero when no campaign exists', async () => {
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
      expect(screen.getByRole('heading', { name: /no campaign yet/i })).toBeTruthy();
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
    const head = await screen.findByRole('button', { name: /^Rules/ });
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

  async function openDeleteDialog(draftCoordinator = new DraftCoordinator()) {
    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Hollow Reach', '5e')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
        draftCoordinator,
      },
    });
    await fireEvent.click(screen.getByText(/Manage campaigns/));
    const row = screen
      .getAllByText('Hollow Reach')
      .map((el) => el.closest('.manage-row'))
      .find((el): el is HTMLElement => el !== null) as HTMLElement;
    await fireEvent.click(within(row).getByTitle('Delete'));
    return screen.findByRole('dialog', { name: /delete campaign/i });
  }

  it('delete opens a dialog offering cascade and convert', async () => {
    const dialog = await openDeleteDialog();
    expect(dialog.getAttribute('tabindex')).toBe('-1');
    expect(screen.getByText('Delete campaign and its notes')).toBeTruthy();
    expect(screen.getByText('Keep notes as a regular collection')).toBeTruthy();
    expect(m.deleteCampaign).not.toHaveBeenCalled();
  });

  it('cascade choice forwards mode "delete"', async () => {
    await openDeleteDialog();
    await fireEvent.click(screen.getByText('Delete campaign and its notes'));
    await waitFor(() => expect(m.deleteCampaign).toHaveBeenCalledWith('camp-1', 'delete'));
  });

  it('keep-notes choice forwards mode "convert_to_regular"', async () => {
    await openDeleteDialog();
    await fireEvent.click(screen.getByText('Keep notes as a regular collection'));
    await waitFor(() =>
      expect(m.deleteCampaign).toHaveBeenCalledWith('camp-1', 'convert_to_regular'),
    );
  });

  it('cancel closes the dialog without deleting', async () => {
    await openDeleteDialog();
    await fireEvent.click(screen.getByText('Cancel'));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(m.deleteCampaign).not.toHaveBeenCalled();
  });

  it('escape closes the dialog without deleting', async () => {
    const dialog = await openDeleteDialog();
    await fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(m.deleteCampaign).not.toHaveBeenCalled();
  });

  it('focuses safe cancellation, traps modal keys, and restores the delete opener', async () => {
    renderView();
    await fireEvent.click(screen.getByText(/Manage campaigns/));
    const row = screen.getAllByText('Reach')[1]?.closest('.manage-row');
    if (!(row instanceof HTMLElement)) throw new Error('Expected the managed campaign row');
    const opener = within(row).getByTitle('Delete');
    opener.focus();
    await fireEvent.click(opener);
    const dialog = await screen.findByRole('dialog', { name: /delete campaign/i });

    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    const escapedKey = vi.fn();
    window.addEventListener('keydown', escapedKey);
    await fireEvent.keyDown(dialog, { key: 'g' });
    expect(escapedKey).not.toHaveBeenCalled();
    window.removeEventListener('keydown', escapedKey);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(opener).toHaveFocus();
  });

  it('deletes using immutable scalar identity captured when confirmation opens', async () => {
    const target = camp('camp-original', 'Original name');
    renderView({ activeCampaignId: 'camp-original', campaigns: [target] });
    await fireEvent.click(screen.getByText(/Manage campaigns/));
    const row = screen.getAllByText('Original name')[1]?.closest('.manage-row');
    if (!(row instanceof HTMLElement)) throw new Error('Expected the managed campaign row');
    await fireEvent.click(within(row).getByTitle('Delete'));

    target.id = 'camp-mutated';
    target.name = 'Mutated name';
    await fireEvent.click(screen.getByText('Delete campaign and its notes'));

    await waitFor(() => expect(m.deleteCampaign).toHaveBeenCalledWith('camp-original', 'delete'));
  });

  it('blocks campaign deletion while a matching draft save is active', async () => {
    const coordinator = new DraftCoordinator();
    const save = deferred<{ notes: string }>();
    const scope = entityScope('camp-1', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'Saved' });
    coordinator.revise(scope, { notes: 'Saving' });
    const saving = coordinator.requestSave(scope, () => save.promise);

    const dialog = await openDeleteDialog(coordinator);
    expect(within(dialog).getByText(/wait for campaign saves to finish/i)).toBeVisible();
    expect(
      within(dialog).getByRole('button', { name: 'Delete campaign and its notes' }),
    ).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: 'Keep notes as a regular collection' }),
    ).toBeDisabled();
    expect(m.deleteCampaign).not.toHaveBeenCalled();

    save.resolve({ notes: 'Saving' });
    await saving;
    await waitFor(() =>
      expect(
        within(dialog).getByRole('button', { name: 'Delete campaign and its notes' }),
      ).toBeEnabled(),
    );
  });

  it('discloses retained draft disposal and preserves every scope when deletion fails', async () => {
    const coordinator = new DraftCoordinator();
    const campaignDraft = entityScope('camp-1', 'npc', 'mira');
    const unrelatedDraft = entityScope('camp-2', 'npc', 'mira');
    coordinator.open(campaignDraft, 'entity:npc:mira', { notes: 'Saved' });
    coordinator.revise(campaignDraft, { notes: 'Retained campaign work' });
    coordinator.open(unrelatedDraft, 'entity:npc:mira', { notes: 'Other saved' });
    coordinator.revise(unrelatedDraft, { notes: 'Other campaign work' });
    m.deleteCampaign.mockRejectedValueOnce(new Error('database unavailable'));

    const dialog = await openDeleteDialog(coordinator);
    expect(within(dialog).getByText(/discard 1 retained draft/i)).toBeVisible();
    await fireEvent.click(within(dialog).getByText('Delete campaign and its notes'));

    await screen.findByText(/database unavailable/i);
    expect(coordinator.get(campaignDraft)?.value).toEqual({ notes: 'Retained campaign work' });
    expect(coordinator.get(unrelatedDraft)?.value).toEqual({ notes: 'Other campaign work' });
  });

  it('purges only the captured campaign drafts and rule recovery after successful deletion', async () => {
    const coordinator = new DraftCoordinator();
    const campaignScopes = [
      oracleScope('camp-1'),
      entityScope('camp-1', 'npc', 'mira'),
      newEntityScope('camp-1', 'npc', 'client-one'),
      sessionScope('camp-1', 'session-one'),
      ruleScope('camp-1', 'rules', 'initiative'),
    ];
    const unrelatedScopes = [
      oracleScope(null),
      entityScope('camp-10', 'npc', 'mira'),
      ruleScope('camp-2', 'rules', 'initiative'),
    ];
    for (const scope of [...campaignScopes, ...unrelatedScopes]) {
      coordinator.open(scope, scope.startsWith('oracle:') ? null : `target:${scope}`, {
        notes: 'Saved',
      });
      coordinator.revise(scope, { notes: `Retained ${scope}` });
    }
    const ruleDraft = ruleScope('camp-1', 'rules', 'initiative');
    rememberRuleRecovery(coordinator, ruleDraft, {
      ruleId: 'initiative',
      title: 'Initiative',
      collectionId: 'rules',
    });
    m.deleteCampaign.mockResolvedValueOnce(undefined);

    const dialog = await openDeleteDialog(coordinator);
    await fireEvent.click(within(dialog).getByText('Delete campaign and its notes'));
    await waitFor(() => expect(m.deleteCampaign).toHaveBeenCalledWith('camp-1', 'delete'));

    for (const scope of campaignScopes) expect(coordinator.get(scope)).toBeUndefined();
    for (const scope of unrelatedScopes) expect(coordinator.get(scope)).toBeDefined();
    expect(rememberedRuleRecovery(coordinator, ruleDraft)).toBeUndefined();
  });

  it('purges only failed new-session attempts for the deleted campaign', async () => {
    const coordinator = new DraftCoordinator();
    const deletedAttempt = newSessionScope('camp-1', 'attempt-one');
    const unrelatedAttempt = newSessionScope('camp-2', 'attempt-two');
    const input = { sessionNumber: 2, title: 'Session 2', datePlayed: '2026-09-14', notes: '' };
    for (const scope of [deletedAttempt, unrelatedAttempt]) {
      coordinator.open(scope, scope, { sessionNumber: 0, title: '', datePlayed: '', notes: '' });
      coordinator.revise(scope, input);
      await coordinator.requestSave(scope, () => Promise.reject(new Error('create failed')));
    }
    m.deleteCampaign.mockResolvedValueOnce(undefined);

    const dialog = await openDeleteDialog(coordinator);
    await fireEvent.click(within(dialog).getByText('Delete campaign and its notes'));
    await waitFor(() => expect(m.deleteCampaign).toHaveBeenCalledWith('camp-1', 'delete'));

    expect(coordinator.get(deletedAttempt)).toBeUndefined();
    expect(coordinator.get(unrelatedAttempt)?.error).toBe('create failed');
  });

  it('fails closed when a matching save starts before post-delete cleanup', async () => {
    const coordinator = new DraftCoordinator();
    const deletion = deferred<undefined>();
    const save = deferred<{ notes: string }>();
    const scope = entityScope('camp-1', 'npc', 'mira');
    coordinator.open(scope, 'entity:npc:mira', { notes: 'Saved' });
    m.deleteCampaign.mockReturnValueOnce(deletion.promise);
    const dialog = await openDeleteDialog(coordinator);
    await fireEvent.click(within(dialog).getByText('Delete campaign and its notes'));
    coordinator.revise(scope, { notes: 'Late saving revision' });
    const saving = coordinator.requestSave(scope, () => save.promise);

    deletion.resolve(undefined);
    await screen.findByText(/could not safely release its retained drafts/i);
    const retainedDialog = screen.getByRole('dialog', { name: /delete campaign/i });
    expect(retainedDialog).toBeVisible();
    expect(coordinator.get(scope)).toBeDefined();

    await fireEvent.click(within(retainedDialog).getByRole('button', { name: 'Cancel' }));
    await fireEvent.keyDown(retainedDialog, { key: 'Escape' });
    expect(screen.getByRole('dialog', { name: /delete campaign/i })).toBeVisible();

    save.resolve({ notes: 'Late saving revision' });
    await saving;

    await fireEvent.click(
      within(retainedDialog).getByRole('button', { name: 'Delete campaign and its notes' }),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(m.deleteCampaign).toHaveBeenCalledTimes(1);
    expect(coordinator.get(scope)).toBeUndefined();
  });

  it('shows a stale badge and compile button per collection', async () => {
    m.getCollections.mockResolvedValue([col('c-1', 'World Guide')]);
    m.getCampaignCollections.mockResolvedValue([col('c-1', 'World Guide')]);
    m.getCodexStatus.mockResolvedValue({
      stale_entities: 12,
      total_entities: 40,
      rules_stale: 0,
      rule_entries: 0,
    });
    renderView();
    await waitFor(() => expect(screen.getByText('12 stale')).toBeTruthy());
    expect(screen.getByLabelText('Compile World Guide')).toBeTruthy();
  });

  it('compile button invokes compileCollection and refreshes status', async () => {
    m.getCollections.mockResolvedValue([col('c-1', 'World Guide')]);
    m.getCampaignCollections.mockResolvedValue([col('c-1', 'World Guide')]);
    m.getCodexStatus.mockResolvedValue({
      stale_entities: 1,
      total_entities: 1,
      rules_stale: 0,
      rule_entries: 0,
    });
    m.compileCollection.mockResolvedValue({
      articles_compiled: 1,
      remaining_stale: 0,
      entries_created: 0,
      entries_updated: 0,
    });
    renderView();
    await fireEvent.click(await screen.findByLabelText('Compile World Guide'));
    await waitFor(() => expect(m.compileCollection).toHaveBeenCalledWith('c-1'));
  });
});
