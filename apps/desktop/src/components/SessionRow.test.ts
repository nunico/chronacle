import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SessionRow from './SessionRow.svelte';
import type { Session } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import { oracleScope, sessionScope, statusOf } from '../lib/drafts/draft-state';
import { i18n } from '../lib/locale.svelte';

vi.mock('../lib/commands', () => ({
  updateSession: vi.fn(),
  deleteSession: vi.fn(),
  getSessionEntities: vi.fn().mockResolvedValue([]),
}));

import * as commands from '../lib/commands';

const mockSession = (overrides: Partial<Session> = {}): Session => ({
  id: 'sess1',
  campaign_id: 'camp1',
  session_number: 3,
  title: 'The Battle of Ashfields',
  date_played: '2026-06-05',
  notes: 'The party fought bravely.',
  created_at: null,
  updated_at: null,
  ...overrides,
});

const emptyEntityMap = new Map<string, { id: string; kind: string }>();

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function renderRow(
  coordinator = new DraftCoordinator(),
  overrides: Partial<{
    campaignId: string;
    session: Session;
    onUpdate: (session: Session) => void;
    onDelete: (id: string) => void;
  }> = {},
) {
  return render(SessionRow, {
    props: {
      campaignId: 'camp1',
      session: mockSession(),
      entityMap: emptyEntityMap,
      onUpdate: vi.fn(),
      onDelete: vi.fn(),
      draftCoordinator: coordinator,
      ...overrides,
    },
  } as never);
}

async function expandRow(title = 'The Battle of Ashfields') {
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(title, 'i') }));
  return screen.findByRole('textbox', { name: 'Name' });
}

describe('SessionRow', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    i18n.setLocale('en');
    vi.mocked(commands.getSessionEntities).mockResolvedValue([]);
  });

  it('renders collapsed state with session title', () => {
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete: vi.fn(),
      },
    });
    expect(screen.getByText('The Battle of Ashfields')).toBeTruthy();
    // Date is visible in collapsed header (formatted or raw)
    expect(screen.getByText(/2026|Jun/)).toBeTruthy();
    // Notes textarea is not rendered when collapsed
    expect(screen.queryByRole('textbox', { name: /notes/i })).toBeNull();
  });

  it('expands on click and shows notes textarea', async () => {
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete: vi.fn(),
      },
    });

    const header = screen.getByRole('button', { name: /The Battle of Ashfields/i });
    await fireEvent.click(header);

    await waitFor(() => {
      expect(screen.getByRole('textbox', { name: /notes/i })).toBeTruthy();
    });
  });

  it('calls onDelete after confirm', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(commands.deleteSession).mockResolvedValue(undefined);

    const onDelete = vi.fn();
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete,
      },
    });

    // Expand first
    const header = screen.getByRole('button', { name: /The Battle of Ashfields/i });
    await fireEvent.click(header);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /delete/i })).toBeTruthy();
    });

    await fireEvent.click(screen.getByRole('button', { name: /delete/i }));

    await waitFor(() => {
      expect(commands.deleteSession).toHaveBeenCalledWith('sess1');
      expect(onDelete).toHaveBeenCalledWith('sess1');
    });
  });

  it('renders WikiText preview when notes present', async () => {
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete: vi.fn(),
      },
    });

    // Expand the row
    const header = screen.getByRole('button', { name: /The Battle of Ashfields/i });
    await fireEvent.click(header);

    await waitFor(() => {
      // The wiki-preview div should be present since notes is non-empty
      const preview = document.querySelector('.wiki-preview');
      expect(preview).not.toBeNull();
      // The text from notes is rendered inside the WikiText component
      expect(preview?.textContent).toContain('The party fought bravely.');
    });
  });

  it('announces pending, saving, and acknowledged saved state for an automatic save', async () => {
    const save = deferred<Session>();
    vi.mocked(commands.updateSession).mockReturnValue(save.promise);
    const onUpdate = vi.fn();
    render(SessionRow, {
      props: {
        campaignId: 'camp1',
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate,
        onDelete: vi.fn(),
        draftCoordinator: new DraftCoordinator(),
      },
    } as never);

    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'A canonical dawn' } });
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');

    await fireEvent.blur(title);
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(1));
    expect(screen.getByRole('status')).toHaveTextContent('Saving…');

    const canonical = { ...mockSession(), title: 'A Canonical Dawn' };
    save.resolve(canonical);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(title).toHaveValue('A Canonical Dawn');
    expect(onUpdate).toHaveBeenCalledWith(canonical);
  });

  it('does not queue a newer revision when the close decision blurs the active field', async () => {
    const firstSave = deferred<Session>();
    vi.mocked(commands.updateSession).mockReturnValue(firstSave.promise);
    const coordinator = new DraftCoordinator();
    renderRow(coordinator);
    const title = await expandRow();

    await fireEvent.input(title, { target: { value: 'Earlier revision' } });
    await fireEvent.blur(title);
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledOnce());
    await fireEvent.input(title, { target: { value: 'Newer retained revision' } });

    coordinator.beginCloseDecision();
    await fireEvent.blur(title, { relatedTarget: null });
    firstSave.resolve({ ...mockSession(), title: 'Earlier revision' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes'));

    expect(commands.updateSession).toHaveBeenCalledOnce();
    expect(title).toHaveValue('Newer retained revision');
    expect(coordinator.canDiscard(sessionScope('camp1', 'sess1'))).toBe(true);
  });

  it('retains a focused edit when navigation unmounts it without a related target', async () => {
    const coordinator = new DraftCoordinator();
    const rendered = renderRow(coordinator);
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Retained across navigation' } });

    const endNavigation = coordinator.beginNavigationTransition();
    await fireEvent.blur(title, { relatedTarget: null });
    rendered.unmount();
    endNavigation();

    expect(commands.updateSession).not.toHaveBeenCalled();
    const retained = coordinator.get(sessionScope('camp1', 'sess1'));
    if (!retained) throw new Error('Expected the session draft to remain available');
    expect(statusOf(retained)).toBe('pending');

    renderRow(coordinator);
    expect(await expandRow('Retained across navigation')).toHaveValue('Retained across navigation');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
  });

  it('still autosaves a concrete within-row blur during an active navigation transition', async () => {
    vi.mocked(commands.updateSession).mockResolvedValue({
      ...mockSession(),
      title: 'Ordinary concrete blur',
    });
    const coordinator = new DraftCoordinator();
    renderRow(coordinator);
    const title = await expandRow();
    const date = screen.getByLabelText('Date played');
    await fireEvent.input(title, { target: { value: 'Ordinary concrete blur' } });

    const endNavigation = coordinator.beginNavigationTransition();
    await fireEvent.blur(title, { relatedTarget: date });
    endNavigation();

    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledOnce());
  });

  it('resumes ordinary blur saving after the close decision is cancelled', async () => {
    vi.mocked(commands.updateSession).mockResolvedValue({
      ...mockSession(),
      title: 'Saved after cancelling close',
    });
    const coordinator = new DraftCoordinator();
    renderRow(coordinator);
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Saved after cancelling close' } });

    coordinator.beginCloseDecision();
    await fireEvent.blur(title, { relatedTarget: null });
    expect(commands.updateSession).not.toHaveBeenCalled();

    coordinator.endCloseDecision();
    title.focus();
    await fireEvent.blur(title, { relatedTarget: null });
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledOnce());
  });

  it('retains all edited fields after failure and retries the intended session with latest content', async () => {
    vi.mocked(commands.updateSession)
      .mockRejectedValueOnce({ code: 'DATABASE', message: 'Session write failed.' })
      .mockResolvedValueOnce({
        ...mockSession(),
        title: 'Ashes after the Storm',
        date_played: '2026-06-06',
        notes: 'The road is flooded.',
      });
    renderRow();
    const user = userEvent.setup();
    const title = await expandRow();
    const date = screen.getByLabelText('Date played');
    const notes = screen.getByRole('textbox', { name: 'Notes' });

    await fireEvent.input(title, { target: { value: 'Ashes after the Storm' } });
    await fireEvent.input(date, { target: { value: '2026-06-06' } });
    await fireEvent.input(notes, { target: { value: 'The road is flooded.' } });
    await fireEvent.blur(notes);

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent("Couldn't save");
    expect(alert).toHaveTextContent('Session write failed.');
    expect(title).toHaveValue('Ashes after the Storm');
    expect(date).toHaveValue('2026-06-06');
    expect(notes).toHaveValue('The road is flooded.');

    const retry = screen.getByRole('button', { name: 'Retry' });
    retry.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]).toEqual([
      'sess1',
      {
        sessionNumber: 3,
        title: 'Ashes after the Storm',
        datePlayed: '2026-06-06',
        notes: 'The road is flooded.',
      },
    ]);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(title).toHaveFocus();
  });

  it('focuses the stable header when keyboard Retry succeeds from a collapsed failed row', async () => {
    vi.mocked(commands.updateSession)
      .mockRejectedValueOnce({ code: 'DATABASE', message: 'Session write failed.' })
      .mockResolvedValueOnce({
        ...mockSession(),
        title: 'Canonical title after collapsed Retry',
      });
    renderRow();
    const user = userEvent.setup();
    const title = await expandRow();

    await user.clear(title);
    await user.type(title, 'Retry this collapsed title');
    await fireEvent.blur(title);
    await screen.findByRole('alert');

    await fireEvent.click(screen.getByRole('button', { name: /Retry this collapsed title/i }));
    expect(screen.queryByRole('textbox', { name: 'Name' })).not.toBeInTheDocument();

    const retry = screen.getByRole('button', { name: 'Retry' });
    retry.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull());
    expect(
      screen.getByRole('button', { name: /Canonical title after collapsed Retry/i }),
    ).toHaveFocus();
  });

  it('does not let unchanged-field blur resubmit a failed revision before keyboard Retry', async () => {
    const retrySave = deferred<Session>();
    vi.mocked(commands.updateSession)
      .mockRejectedValueOnce({ code: 'DATABASE', message: 'Session write failed.' })
      .mockReturnValueOnce(retrySave.promise);
    renderRow();
    const user = userEvent.setup();
    const title = await expandRow();

    await user.click(title);
    await user.clear(title);
    await user.type(title, 'Retry stays actionable');
    title.blur();

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Session write failed.');
    expect(title).toHaveValue('Retry stays actionable');
    expect(commands.updateSession).toHaveBeenCalledTimes(1);

    await user.click(title);
    expect(title).toHaveFocus();
    const retry = screen.getByRole('button', { name: 'Retry' });
    retry.focus();
    await Promise.resolve();

    expect(commands.updateSession).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('alert')).toHaveTextContent('Session write failed.');
    expect(title).toHaveValue('Retry stays actionable');
    expect(retry).toHaveFocus();

    await user.keyboard('{Enter}');
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]).toEqual([
      'sess1',
      {
        sessionNumber: 3,
        title: 'Retry stays actionable',
        datePlayed: '2026-06-05',
        notes: 'The party fought bravely.',
      },
    ]);

    retrySave.resolve({ ...mockSession(), title: 'Retry Saved Canonically' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(title).toHaveValue('Retry Saved Canonically');
    expect(title).toHaveFocus();
  });

  it('keeps a newer revision pending when an older acknowledgment has equal canonical content', async () => {
    const first = deferred<Session>();
    const second = deferred<Session>();
    vi.mocked(commands.updateSession)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    renderRow();
    const title = await expandRow();

    await fireEvent.input(title, { target: { value: 'Revision one' } });
    await fireEvent.blur(title);
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(1));
    await fireEvent.input(title, { target: { value: 'Revision two' } });

    first.resolve({ ...mockSession(), title: 'Revision two' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes'));
    expect(title).toHaveValue('Revision two');
    expect(screen.queryByText('Saved', { exact: true })).not.toBeInTheDocument();

    await fireEvent.blur(title);
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    second.resolve({ ...mockSession(), title: 'Revision two' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
  });

  it('serializes rapid saves and coalesces queued work to the newest revision', async () => {
    const first = deferred<Session>();
    const latest = deferred<Session>();
    vi.mocked(commands.updateSession)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(latest.promise);
    renderRow();
    const title = await expandRow();

    for (const value of ['First rapid title', 'Second rapid title', 'Newest rapid title']) {
      await fireEvent.input(title, { target: { value } });
      await fireEvent.blur(title);
    }
    expect(commands.updateSession).toHaveBeenCalledTimes(1);

    first.resolve({ ...mockSession(), title: 'First rapid title' });
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]?.[1].title).toBe('Newest rapid title');
    expect(title).toHaveValue('Newest rapid title');
    expect(screen.getByRole('status')).toHaveTextContent('Saving…');

    latest.resolve({ ...mockSession(), title: 'Newest rapid title' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
  });

  it('restores a retained draft after remount without leaking it to another session or campaign', async () => {
    const coordinator = new DraftCoordinator();
    const first = renderRow(coordinator);
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Retained Camp One title' } });
    first.unmount();

    const sameScope = renderRow(coordinator);
    expect(await expandRow()).toHaveValue('Retained Camp One title');
    sameScope.unmount();

    const otherSession = mockSession({ id: 'sess2', title: 'Another session' });
    const secondScope = renderRow(coordinator, { session: otherSession });
    expect(await expandRow('Another session')).toHaveValue('Another session');
    secondScope.unmount();

    const otherCampaign = mockSession({ campaign_id: 'camp2' });
    renderRow(coordinator, { campaignId: 'camp2', session: otherCampaign });
    expect(await expandRow()).toHaveValue('The Battle of Ashfields');
  });

  it('clears pending state without writing when an edit returns to the saved baseline', async () => {
    renderRow();
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Temporary title' } });
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    await fireEvent.input(title, { target: { value: mockSession().title } });
    await fireEvent.blur(title);

    expect(screen.queryByText('Unsaved changes', { exact: true })).not.toBeInTheDocument();
    expect(commands.updateSession).not.toHaveBeenCalled();
  });

  it('localizes an unavailable-session failure and keeps Retry on the original target', async () => {
    const backendDetail = 'Internal session row 41 vanished during UPDATE.';
    vi.mocked(commands.updateSession)
      .mockRejectedValueOnce({ code: 'NOT_FOUND', message: backendDetail })
      .mockRejectedValueOnce({ code: 'NOT_FOUND', message: backendDetail });
    renderRow();
    const user = userEvent.setup();
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Last local title' } });
    await fireEvent.blur(title);

    const retry = await screen.findByRole('button', { name: 'Retry' });
    expect(screen.getByRole('alert')).toHaveTextContent('This record is no longer available.');
    expect(screen.getByRole('alert')).not.toHaveTextContent(backendDetail);
    retry.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]?.[0]).toBe('sess1');
    expect(title).toHaveValue('Last local title');
    expect(screen.getByRole('button', { name: 'Retry' })).toHaveFocus();
  });

  it('shows the localized unavailable-target recovery in German without backend detail', async () => {
    const backendDetail = 'Internal session row 41 vanished during UPDATE.';
    i18n.setLocale('de');
    vi.mocked(commands.updateSession).mockRejectedValue({
      code: 'NOT_FOUND',
      message: backendDetail,
    });
    renderRow();
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Letzter lokaler Titel' } });
    await fireEvent.blur(title);

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Dieser Eintrag ist nicht mehr verfügbar.');
    expect(alert).not.toHaveTextContent(backendDetail);
    expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeEnabled();
    expect(title).toHaveValue('Letzter lokaler Titel');
  });

  it('rejects a mismatched session acknowledgment and retries the original target', async () => {
    const coordinator = new DraftCoordinator();
    vi.mocked(commands.updateSession)
      .mockResolvedValueOnce(
        mockSession({
          id: 'sess2',
          title: 'Wrong-record canonical title must not be acknowledged',
        }),
      )
      .mockResolvedValueOnce(mockSession({ title: 'Identity-safe session title' }));
    renderRow(coordinator);
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Identity-safe session title' } });
    await fireEvent.blur(title);

    const failure = await screen.findByRole('alert');
    expect(failure).toHaveTextContent("Couldn't save");
    expect(screen.getByRole('button', { name: 'Retry' })).toBeEnabled();
    expect(title).toHaveValue('Identity-safe session title');
    expect(screen.queryByText('Saved', { exact: true })).not.toBeInTheDocument();
    const scope = sessionScope('camp1', 'sess1');
    const failedDraft = coordinator.get<{ title: string }>(scope);
    if (!failedDraft) throw new Error('Expected the original session draft to remain open');
    expect(failedDraft.target).toBe('session:sess1');
    expect(failedDraft.value.title).toBe('Identity-safe session title');
    expect(statusOf(failedDraft)).toBe('failed');

    await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]).toEqual([
      'sess1',
      {
        sessionNumber: 3,
        title: 'Identity-safe session title',
        datePlayed: '2026-06-05',
        notes: 'The party fought bravely.',
      },
    ]);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(title).toHaveValue('Identity-safe session title');
  });

  it('prevents editing and recovery writes while confirmed deletion is pending', async () => {
    const deletion = deferred<undefined>();
    vi.mocked(commands.updateSession).mockRejectedValueOnce({
      code: 'DATABASE',
      message: 'Session write failed.',
    });
    vi.mocked(commands.deleteSession).mockReturnValue(deletion.promise);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const coordinator = new DraftCoordinator();
    coordinator.open(oracleScope('camp1'), null, 'Unsent unrelated question');
    coordinator.open(sessionScope('camp1', 'sess2'), 'session:sess2', {
      sessionNumber: 4,
      title: 'Unrelated session',
      datePlayed: '2026-06-07',
      notes: 'Unrelated retained notes.',
    });
    coordinator.revise(sessionScope('camp1', 'sess2'), {
      sessionNumber: 4,
      title: 'Unrelated retained session',
      datePlayed: '2026-06-07',
      notes: 'Unrelated retained notes.',
    });
    renderRow(coordinator, { session: mockSession() });
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Delete this failed draft' } });
    await fireEvent.blur(title);
    const retry = await screen.findByRole('button', { name: 'Retry' });
    const discard = screen.getByRole('button', { name: 'Discard changes' });

    await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(commands.deleteSession).toHaveBeenCalledWith('sess1'));

    expect(title).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(retry).toBeDisabled();
    expect(discard).toBeDisabled();
    await fireEvent.click(retry);
    await fireEvent.blur(title);
    expect(commands.updateSession).toHaveBeenCalledTimes(1);

    deletion.resolve(undefined);
    await waitFor(() => {
      expect(coordinator.get(sessionScope('camp1', 'sess1'))).toBeUndefined();
    });
    expect(coordinator.get<string>(oracleScope('camp1'))?.value).toBe('Unsent unrelated question');
    expect(coordinator.get(sessionScope('camp1', 'sess2'))?.value).toMatchObject({
      title: 'Unrelated retained session',
    });
  });

  it('restores failed-draft recovery when a confirmed deletion is rejected', async () => {
    const deletion = deferred<undefined>();
    const deleteError = { code: 'DATABASE', message: 'Session delete failed.' };
    vi.mocked(commands.updateSession).mockRejectedValueOnce({
      code: 'DATABASE',
      message: 'Session write failed.',
    });
    vi.mocked(commands.deleteSession).mockReturnValue(deletion.promise);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const coordinator = new DraftCoordinator();
    coordinator.open(oracleScope('camp1'), null, 'Unsent unrelated question');
    const onDelete = vi.fn();

    try {
      renderRow(coordinator, { onDelete });
      const title = await expandRow();
      const date = screen.getByLabelText('Date played');
      const notes = screen.getByRole('textbox', { name: 'Notes' });
      await fireEvent.input(title, { target: { value: 'Retained after rejected deletion' } });
      await fireEvent.input(date, { target: { value: '2026-06-08' } });
      await fireEvent.input(notes, { target: { value: 'Deletion must not erase these notes.' } });
      await fireEvent.blur(notes);
      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('Session write failed.');

      const retry = screen.getByRole('button', { name: 'Retry' });
      const discard = screen.getByRole('button', { name: 'Discard changes' });
      const deleteButton = screen.getByRole('button', { name: 'Delete' });
      await fireEvent.click(deleteButton);
      await waitFor(() => expect(commands.deleteSession).toHaveBeenCalledWith('sess1'));

      expect(title).toBeDisabled();
      expect(date).toBeDisabled();
      expect(notes).toBeDisabled();
      expect(deleteButton).toBeDisabled();
      expect(retry).toBeDisabled();
      expect(discard).toBeDisabled();
      await fireEvent.click(retry);
      await fireEvent.blur(title);
      expect(commands.updateSession).toHaveBeenCalledTimes(1);

      deletion.reject(deleteError);
      await waitFor(() => {
        expect(errorSpy).toHaveBeenCalledWith('Failed to delete session:', deleteError);
      });

      expect(title).toBeEnabled();
      expect(date).toBeEnabled();
      expect(notes).toBeEnabled();
      expect(deleteButton).toBeEnabled();
      expect(retry).toBeEnabled();
      expect(discard).toBeEnabled();
      expect(alert).toHaveTextContent('Session write failed.');
      expect(title).toHaveValue('Retained after rejected deletion');
      expect(date).toHaveValue('2026-06-08');
      expect(notes).toHaveValue('Deletion must not erase these notes.');
      expect(commands.updateSession).toHaveBeenCalledTimes(1);
      expect(onDelete).not.toHaveBeenCalled();

      const retained = coordinator.get(sessionScope('camp1', 'sess1'));
      expect(retained).toBeDefined();
      if (!retained) throw new Error('Expected the rejected deletion to retain its session draft');
      expect(statusOf(retained)).toBe('failed');
      expect(retained.value).toMatchObject({
        title: 'Retained after rejected deletion',
        datePlayed: '2026-06-08',
        notes: 'Deletion must not erase these notes.',
      });
      expect(coordinator.get<string>(oracleScope('camp1'))?.value).toBe(
        'Unsent unrelated question',
      );
    } finally {
      errorSpy.mockRestore();
    }
  });

  it('restores the saved baseline and focuses the stable header after keyboard Discard', async () => {
    const coordinator = new DraftCoordinator();
    coordinator.open(oracleScope('camp1'), null, 'Unsent unrelated question');
    renderRow(coordinator);
    const user = userEvent.setup();
    const title = await expandRow();
    await user.clear(title);
    await user.type(title, 'Throw away this title');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');

    const discard = screen.getByRole('button', { name: 'Discard changes' });
    discard.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(title).toHaveValue('The Battle of Ashfields'));
    expect(commands.updateSession).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: /The Battle of Ashfields/i })).toHaveFocus();
    expect(coordinator.get<string>(oracleScope('camp1'))?.value).toBe('Unsent unrelated question');
  });

  it('blocks Delete during an active save, then removes only that settled session draft', async () => {
    const save = deferred<Session>();
    vi.mocked(commands.updateSession).mockReturnValue(save.promise);
    vi.mocked(commands.deleteSession).mockResolvedValue(undefined);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const coordinator = new DraftCoordinator();
    coordinator.open(oracleScope('camp1'), null, 'Unsent unrelated question');
    renderRow(coordinator);
    const title = await expandRow();
    await fireEvent.input(title, { target: { value: 'Saving before deletion' } });
    await fireEvent.blur(title);
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledOnce());

    const deleteButton = screen.getByRole('button', { name: 'Delete' });
    expect(deleteButton).toBeDisabled();
    expect(deleteButton).toHaveAccessibleDescription(
      'Wait for saving to finish before discarding changes',
    );
    await fireEvent.click(deleteButton);
    expect(commands.deleteSession).not.toHaveBeenCalled();

    save.resolve({ ...mockSession(), title: 'Saving before deletion' });
    await waitFor(() => expect(deleteButton).toBeEnabled());
    await fireEvent.click(deleteButton);
    await waitFor(() => expect(commands.deleteSession).toHaveBeenCalledWith('sess1'));

    expect(coordinator.get(sessionScope('camp1', 'sess1'))).toBeUndefined();
    expect(coordinator.get<string>(oracleScope('camp1'))?.value).toBe('Unsent unrelated question');
  });
});
