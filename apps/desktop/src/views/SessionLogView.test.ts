import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SessionLogView, * as sessionLogViewModule from './SessionLogView.svelte';
import type { Session } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import { oracleScope, sessionScope } from '../lib/drafts/draft-state';
import { i18n } from '../lib/locale.svelte';

interface SessionLoadFenceContract {
  readonly trackedCount: number;
  begin(request: number, campaignId: string): void;
  acknowledge(campaignId: string, scope: string): void;
  settle(request: number, campaignId: string): Set<string>;
}

const { SessionLoadFence } = sessionLogViewModule as unknown as {
  SessionLoadFence: new () => SessionLoadFenceContract;
};

vi.mock('../lib/commands', () => ({
  getSessions: vi.fn(),
  createSession: vi.fn(),
  getEntities: vi.fn().mockResolvedValue([]),
  updateSession: vi.fn(),
  deleteSession: vi.fn(),
  getSessionEntities: vi.fn().mockResolvedValue([]),
}));

import * as commands from '../lib/commands';

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function session(campaignId: string, title: string): Session {
  return {
    id: campaignId === 'camp-a' ? 'session-a' : 'session-b',
    campaign_id: campaignId,
    session_number: 1,
    title,
    date_played: '2026-09-01',
    notes: `${title} notes`,
    created_at: null,
    updated_at: null,
  };
}

function renderLog(campaignId: string, coordinator = new DraftCoordinator()) {
  return render(SessionLogView, {
    props: { campaignId, draftCoordinator: coordinator },
  } as never);
}

describe('SessionLoadFence', () => {
  it('tracks create acknowledgments only for the active list request', () => {
    const fence = new SessionLoadFence();

    fence.acknowledge('camp-a', 'session:camp-a:without-load');
    expect(fence.trackedCount).toBe(0);

    fence.begin(1, 'camp-a');
    fence.acknowledge('camp-a', 'session:camp-a:during-load');
    expect(fence.trackedCount).toBe(1);
    expect(fence.settle(1, 'camp-a')).toEqual(new Set(['session:camp-a:during-load']));
    expect(fence.trackedCount).toBe(0);
  });
});

describe('SessionLogView draft coordination', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(commands.getEntities).mockResolvedValue([]);
    vi.mocked(commands.getSessionEntities).mockResolvedValue([]);
  });

  afterEach(() => {
    i18n.setLocale('en');
    vi.useRealTimers();
  });

  it('retains a failed create in its campaign and coalesces keyboard Retry', async () => {
    vi.setSystemTime(new Date('2026-09-14T12:00:00Z'));
    const retryCreate = deferred<Session>();
    const coordinator = new DraftCoordinator();
    const created = {
      ...session('camp-a', 'Session 2'),
      id: 'created-session-a',
      session_number: 2,
      date_played: '2026-09-14',
      notes: '',
    };
    vi.mocked(commands.getSessions).mockImplementation((campaignId) =>
      Promise.resolve([session(campaignId, campaignId === 'camp-a' ? 'Ashes at Dawn' : 'Rain')]),
    );
    vi.mocked(commands.createSession)
      .mockRejectedValueOnce({ code: 'DATABASE', message: 'Session create failed.' })
      .mockReturnValueOnce(retryCreate.promise);
    const user = userEvent.setup();

    const first = renderLog('camp-a', coordinator);
    await screen.findByText('Ashes at Dawn');
    await user.click(screen.getByRole('button', { name: /New session/i }));

    const failure = await screen.findByRole('alert');
    expect(failure).toHaveTextContent("Couldn't create session.");
    expect(failure).toHaveTextContent('Session create failed.');
    expect(screen.getByRole('button', { name: 'Retry' })).toBeVisible();
    expect(screen.getByRole('button', { name: 'Discard changes' })).toBeVisible();
    expect(vi.mocked(commands.createSession).mock.calls[0]).toEqual([
      'camp-a',
      {
        sessionNumber: 2,
        title: 'Session 2',
        datePlayed: '2026-09-14',
        notes: '',
      },
    ]);
    first.unmount();

    const campaignB = renderLog('camp-b', coordinator);
    await screen.findByText('Rain');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    campaignB.unmount();

    renderLog('camp-a', coordinator);
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't create session.");
    const retry = screen.getByRole('button', { name: 'Retry' });
    retry.focus();
    await user.keyboard('{Enter}{Enter}');

    expect(commands.createSession).toHaveBeenCalledTimes(2);
    expect(retry).toBeDisabled();
    expect(vi.mocked(commands.createSession).mock.calls[1]).toEqual(
      vi.mocked(commands.createSession).mock.calls[0],
    );

    retryCreate.resolve(created);
    await waitFor(() => expect(screen.getByRole('button', { name: /Session 2/ })).toBeVisible());
    expect(screen.getAllByRole('button', { name: /Session 2/ })).toHaveLength(1);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('localizes and discards only a failed create with keyboard focus recovery', async () => {
    i18n.setLocale('de');
    const coordinator = new DraftCoordinator();
    const unrelatedScope = oracleScope('camp-a');
    coordinator.open(unrelatedScope, null, 'Unabhängige Oracle-Frage');
    vi.mocked(commands.getSessions).mockResolvedValue([session('camp-a', 'Ashes at Dawn')]);
    vi.mocked(commands.createSession).mockRejectedValue({
      code: 'DATABASE',
      message: 'Sitzung konnte nicht gespeichert werden.',
    });
    const user = userEvent.setup();

    renderLog('camp-a', coordinator);
    await screen.findByText('Ashes at Dawn');
    await user.click(screen.getByRole('button', { name: /Neue Sitzung/i }));

    const failure = await screen.findByRole('alert');
    expect(failure).toHaveTextContent('Sitzung konnte nicht erstellt werden.');
    const discard = screen.getByRole('button', { name: 'Änderungen verwerfen' });
    expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeVisible();
    discard.focus();
    await user.keyboard('{Enter}');

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(commands.createSession).toHaveBeenCalledOnce();
    expect(coordinator.get<string>(unrelatedScope)?.value).toBe('Unabhängige Oracle-Frage');
    expect(screen.getByRole('button', { name: /Neue Sitzung/i })).toHaveFocus();
  });

  it('uses the newest request generation across an A to B to A campaign switch', async () => {
    const firstCampaignA = deferred<Session[]>();
    let campaignARequests = 0;
    vi.mocked(commands.getSessions).mockImplementation((campaignId) => {
      if (campaignId === 'camp-a') {
        campaignARequests += 1;
        return campaignARequests === 1
          ? firstCampaignA.promise
          : Promise.resolve([session('camp-a', 'Newest Campaign A session')]);
      }
      return Promise.resolve([session('camp-b', 'Campaign B session')]);
    });
    const coordinator = new DraftCoordinator();
    const rendered = renderLog('camp-a', coordinator);

    await rendered.rerender({
      campaignId: 'camp-b',
      draftCoordinator: coordinator,
    } as never);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledWith('camp-b'));
    expect(await screen.findByText('Campaign B session')).toBeVisible();

    await rendered.rerender({ campaignId: 'camp-a', draftCoordinator: coordinator } as never);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledTimes(3));
    expect(await screen.findByText('Newest Campaign A session')).toBeVisible();

    firstCampaignA.resolve([session('camp-a', 'Obsolete Campaign A session')]);
    await firstCampaignA.promise;
    await tick();
    expect(screen.queryByText('Obsolete Campaign A session')).not.toBeInTheDocument();
    expect(screen.getByText('Newest Campaign A session')).toBeVisible();
  });

  it('does not integrate a delayed session creation into another campaign', async () => {
    const pendingCreate = deferred<Session>();
    const createdForCampaignA: Session = {
      ...session('camp-a', 'Created for Campaign A'),
      id: 'session-created-a',
    };
    let campaignARequests = 0;
    vi.mocked(commands.getSessions).mockImplementation((campaignId) => {
      if (campaignId === 'camp-a') {
        campaignARequests += 1;
        return Promise.resolve(campaignARequests === 1 ? [] : [createdForCampaignA]);
      }
      return Promise.resolve([session('camp-b', 'Campaign B session')]);
    });
    vi.mocked(commands.createSession).mockReturnValue(pendingCreate.promise);
    const coordinator = new DraftCoordinator();
    const rendered = renderLog('camp-a', coordinator);

    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledWith('camp-a'));
    await fireEvent.click(screen.getByRole('button', { name: /New Session/i }));
    expect(commands.createSession).toHaveBeenCalledWith('camp-a', expect.any(Object));

    await rendered.rerender({ campaignId: 'camp-b', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Campaign B session')).toBeVisible();

    pendingCreate.resolve(createdForCampaignA);
    await pendingCreate.promise;
    await tick();

    expect(screen.queryByText('Created for Campaign A')).not.toBeInTheDocument();
    expect(coordinator.get(sessionScope('camp-b', createdForCampaignA.id))).toBeUndefined();
    expect(coordinator.get(sessionScope('camp-a', createdForCampaignA.id))).toBeUndefined();

    await rendered.rerender({ campaignId: 'camp-a', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Created for Campaign A')).toBeVisible();
    expect(
      coordinator.get<{ title: string }>(sessionScope('camp-a', createdForCampaignA.id))?.value
        .title,
    ).toBe('Created for Campaign A');
  });

  it('keeps a created session when an older same-campaign list completes later', async () => {
    const pendingCreate = deferred<Session>();
    const staleCampaignAList = deferred<Session[]>();
    const createdForCampaignA: Session = {
      ...session('camp-a', 'Created after reload started'),
      id: 'session-created-during-reload',
    };
    let campaignARequests = 0;
    vi.mocked(commands.getSessions).mockImplementation((campaignId) => {
      if (campaignId === 'camp-a') {
        campaignARequests += 1;
        return campaignARequests === 1 ? Promise.resolve([]) : staleCampaignAList.promise;
      }
      return Promise.resolve([session('camp-b', 'Campaign B session')]);
    });
    vi.mocked(commands.createSession).mockReturnValue(pendingCreate.promise);
    const coordinator = new DraftCoordinator();
    const rendered = renderLog('camp-a', coordinator);

    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledWith('camp-a'));
    await fireEvent.click(screen.getByRole('button', { name: /New Session/i }));
    await rendered.rerender({ campaignId: 'camp-b', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Campaign B session')).toBeVisible();

    await rendered.rerender({ campaignId: 'camp-a', draftCoordinator: coordinator } as never);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledTimes(3));

    pendingCreate.resolve(createdForCampaignA);
    await waitFor(() =>
      expect(
        coordinator.get<{ title: string }>(sessionScope('camp-a', createdForCampaignA.id))?.value
          .title,
      ).toBe('Created after reload started'),
    );

    staleCampaignAList.resolve([]);
    await staleCampaignAList.promise;
    await tick();

    expect(screen.getByText('Created after reload started')).toBeVisible();
    expect(
      coordinator.get<{ title: string }>(sessionScope('camp-a', createdForCampaignA.id))?.value
        .title,
    ).toBe('Created after reload started');
  });

  it('keeps a created session when an older list rejects, until a later list omits it', async () => {
    const pendingCreate = deferred<Session>();
    const staleCampaignAList = deferred<Session[]>();
    const createdForCampaignA: Session = {
      ...session('camp-a', 'Created before reload failure'),
      id: 'session-created-before-failure',
    };
    let campaignARequests = 0;
    vi.mocked(commands.getSessions).mockImplementation((campaignId) => {
      if (campaignId === 'camp-a') {
        campaignARequests += 1;
        if (campaignARequests === 1 || campaignARequests === 3) return Promise.resolve([]);
        return staleCampaignAList.promise;
      }
      return Promise.resolve([session('camp-b', 'Campaign B session')]);
    });
    vi.mocked(commands.createSession).mockReturnValue(pendingCreate.promise);
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const coordinator = new DraftCoordinator();
    const rendered = renderLog('camp-a', coordinator);

    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledWith('camp-a'));
    await fireEvent.click(screen.getByRole('button', { name: /New Session/i }));
    await rendered.rerender({ campaignId: 'camp-b', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Campaign B session')).toBeVisible();

    await rendered.rerender({ campaignId: 'camp-a', draftCoordinator: coordinator } as never);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledTimes(3));
    pendingCreate.resolve(createdForCampaignA);
    await waitFor(() =>
      expect(
        coordinator.get<{ title: string }>(sessionScope('camp-a', createdForCampaignA.id))?.value
          .title,
      ).toBe('Created before reload failure'),
    );

    staleCampaignAList.reject(new Error('stale list failed'));
    await expect(staleCampaignAList.promise).rejects.toThrow('stale list failed');
    await tick();
    expect(screen.getByText('Created before reload failure')).toBeVisible();

    await rendered.rerender({ campaignId: 'camp-b', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Campaign B session')).toBeVisible();
    await rendered.rerender({ campaignId: 'camp-a', draftCoordinator: coordinator } as never);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledTimes(5));
    expect(screen.queryByText('Created before reload failure')).not.toBeInTheDocument();
    consoleError.mockRestore();
  });

  it('does not duplicate a session listed before its create acknowledgment', async () => {
    const pendingCreate = deferred<Session>();
    const createdForCampaignA: Session = {
      ...session('camp-a', 'Listed before create acknowledgment'),
      id: 'session-listed-before-create-ack',
    };
    let campaignARequests = 0;
    vi.mocked(commands.getSessions).mockImplementation((campaignId) => {
      if (campaignId === 'camp-a') {
        campaignARequests += 1;
        return Promise.resolve(campaignARequests === 1 ? [] : [createdForCampaignA]);
      }
      return Promise.resolve([session('camp-b', 'Campaign B session')]);
    });
    vi.mocked(commands.createSession).mockReturnValue(pendingCreate.promise);
    const coordinator = new DraftCoordinator();
    const rendered = renderLog('camp-a', coordinator);

    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledWith('camp-a'));
    await fireEvent.click(screen.getByRole('button', { name: /New Session/i }));
    await rendered.rerender({ campaignId: 'camp-b', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Campaign B session')).toBeVisible();
    await rendered.rerender({ campaignId: 'camp-a', draftCoordinator: coordinator } as never);
    expect(await screen.findByText('Listed before create acknowledgment')).toBeVisible();

    pendingCreate.resolve(createdForCampaignA);
    await pendingCreate.promise;
    await tick();

    expect(screen.getAllByText('Listed before create acknowledgment')).toHaveLength(1);
    expect(
      coordinator.get<{ title: string }>(sessionScope('camp-a', createdForCampaignA.id))?.value
        .title,
    ).toBe('Listed before create acknowledgment');
  });

  it('restores a retained session draft after view unmount and an older list reload', async () => {
    const coordinator = new DraftCoordinator();
    vi.mocked(commands.getSessions).mockResolvedValue([session('camp-a', 'Ashes at Dawn')]);
    const first = renderLog('camp-a', coordinator);
    const firstHeader = await screen.findByRole('button', { name: /Ashes at Dawn/ });
    await fireEvent.click(firstHeader);
    await fireEvent.input(screen.getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Ashes retained through navigation' },
    });
    first.unmount();

    renderLog('camp-a', coordinator);
    const reloadedHeader = await screen.findByRole('button', { name: /Ashes at Dawn/ });
    await fireEvent.click(reloadedHeader);
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue(
      'Ashes retained through navigation',
    );
    expect(screen.getByText('Unsaved changes', { exact: true })).toBeVisible();
  });

  it('releases clean session projections when the view unmounts', async () => {
    const coordinator = new DraftCoordinator();
    const scope = sessionScope('camp-a', 'session-a');
    vi.mocked(commands.getSessions).mockResolvedValue([session('camp-a', 'Ashes at Dawn')]);
    const rendered = renderLog('camp-a', coordinator);

    await screen.findByText('Ashes at Dawn');
    expect(coordinator.get(scope)).toBeDefined();
    rendered.unmount();

    expect(coordinator.get(scope)).toBeUndefined();
  });

  it('does not let an obsolete unmounted same-campaign instance reconcile shared draft state', async () => {
    const obsolete = deferred<Session[]>();
    vi.mocked(commands.getSessions)
      .mockReturnValueOnce(obsolete.promise)
      .mockResolvedValueOnce([session('camp-a', 'Current same-campaign session')]);
    const coordinator = new DraftCoordinator();
    const oldInstance = renderLog('camp-a', coordinator);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledTimes(1));
    oldInstance.unmount();

    renderLog('camp-a', coordinator);
    expect(await screen.findByText('Current same-campaign session')).toBeVisible();
    obsolete.resolve([session('camp-a', 'Obsolete unmounted session')]);
    await obsolete.promise;
    await tick();

    expect(screen.queryByText('Obsolete unmounted session')).not.toBeInTheDocument();
    expect(screen.getByText('Current same-campaign session')).toBeVisible();
    expect(
      coordinator.get<{ title: string }>(sessionScope('camp-a', 'session-a'))?.value.title,
    ).toBe('Current same-campaign session');
  });

  it('does not let a list requested before save acknowledgment replace that acknowledgment', async () => {
    const oldReload = deferred<Session[]>();
    const save = deferred<Session>();
    vi.mocked(commands.getSessions)
      .mockResolvedValueOnce([session('camp-a', 'Ashes at Dawn')])
      .mockReturnValueOnce(oldReload.promise)
      .mockResolvedValueOnce([session('camp-a', 'Later authoritative session')]);
    vi.mocked(commands.updateSession).mockReturnValue(save.promise);
    const coordinator = new DraftCoordinator();
    const first = renderLog('camp-a', coordinator);
    const header = await screen.findByRole('button', { name: /Ashes at Dawn/ });
    await fireEvent.click(header);
    const title = screen.getByRole('textbox', { name: 'Name' });
    await fireEvent.input(title, { target: { value: 'Canonical saved session' } });
    await fireEvent.blur(title);
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledOnce());
    first.unmount();

    const preAcknowledgmentReload = renderLog('camp-a', coordinator);
    await waitFor(() => expect(commands.getSessions).toHaveBeenCalledTimes(2));
    save.resolve(session('camp-a', 'Canonical saved session'));
    await waitFor(() =>
      expect(
        coordinator.get<{ title: string }>(sessionScope('camp-a', 'session-a'))?.value.title,
      ).toBe('Canonical saved session'),
    );
    oldReload.resolve([session('camp-a', 'Ashes at Dawn')]);

    expect(await screen.findByText('Canonical saved session')).toBeVisible();
    await fireEvent.click(screen.getByRole('button', { name: /Canonical saved session/ }));
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Canonical saved session');
    expect(screen.getByText('Saved', { exact: true })).toBeVisible();
    preAcknowledgmentReload.unmount();

    renderLog('camp-a', coordinator);
    expect(await screen.findByText('Later authoritative session')).toBeVisible();
    await fireEvent.click(screen.getByRole('button', { name: /Later authoritative session/ }));
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue(
      'Later authoritative session',
    );
  });

  it('presents an at-risk recovery row when the current list omits its session', async () => {
    const coordinator = new DraftCoordinator();
    const scope = sessionScope('camp-a', 'session-a');
    coordinator.open(scope, 'session:session-a', {
      sessionNumber: 1,
      title: 'Missing saved session',
      datePlayed: '2026-09-01',
      notes: 'Saved notes',
    });
    coordinator.revise(scope, {
      sessionNumber: 1,
      title: 'Retained missing session',
      datePlayed: '2026-09-01',
      notes: 'Local notes that must remain available',
    });
    vi.mocked(commands.getSessions).mockResolvedValue([]);

    renderLog('camp-a', coordinator);
    const recovery = await screen.findByRole('button', { name: /Retained missing session/ });
    await fireEvent.click(recovery);
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Retained missing session');
    expect(screen.getByRole('textbox', { name: 'Notes' })).toHaveValue(
      'Local notes that must remain available',
    );
    expect(screen.getByText('Unsaved changes', { exact: true })).toBeVisible();
  });

  it('retains a missing failed session through navigation for targeted Retry and Discard', async () => {
    const backendDetail = 'Internal session row 17 vanished during UPDATE.';
    const coordinator = new DraftCoordinator();
    const missingScope = sessionScope('camp-a', 'session-a');
    const unrelatedScope = oracleScope('camp-a');
    const user = userEvent.setup();
    coordinator.open(unrelatedScope, null, 'Unsent unrelated Oracle question');
    vi.mocked(commands.getSessions)
      .mockResolvedValueOnce([session('camp-a', 'Ashes at Dawn')])
      .mockResolvedValueOnce([]);
    vi.mocked(commands.updateSession).mockRejectedValue({
      code: 'NOT_FOUND',
      message: backendDetail,
    });

    const first = renderLog('camp-a', coordinator);
    const header = await screen.findByRole('button', { name: /Ashes at Dawn/ });
    await fireEvent.click(header);
    await fireEvent.input(screen.getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Ashes retained locally' },
    });
    await fireEvent.input(screen.getByLabelText('Date played'), {
      target: { value: '2026-09-09' },
    });
    await fireEvent.input(screen.getByRole('textbox', { name: 'Notes' }), {
      target: { value: 'Exact notes retained after deletion.' },
    });
    await fireEvent.blur(screen.getByRole('textbox', { name: 'Notes' }));
    const initialFailure = await screen.findByRole('alert');
    expect(initialFailure).toHaveTextContent('This record is no longer available.');
    expect(initialFailure).not.toHaveTextContent(backendDetail);
    first.unmount();

    renderLog('camp-a', coordinator);
    const recoveryHeader = await screen.findByRole('button', { name: /Ashes retained locally/ });
    const collapsedFailure = screen.getByRole('alert');
    expect(collapsedFailure).toHaveTextContent('This record is no longer available.');
    expect(collapsedFailure).not.toHaveTextContent(backendDetail);
    await fireEvent.click(recoveryHeader);
    expect(screen.getByRole('textbox', { name: 'Name' })).toHaveValue('Ashes retained locally');
    expect(screen.getByLabelText('Date played')).toHaveValue('2026-09-09');
    expect(screen.getByRole('textbox', { name: 'Notes' })).toHaveValue(
      'Exact notes retained after deletion.',
    );
    const recoveredFailure = screen.getByRole('alert');
    expect(recoveredFailure).toHaveTextContent('This record is no longer available.');
    expect(recoveredFailure).not.toHaveTextContent(backendDetail);

    await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]).toEqual([
      'session-a',
      {
        sessionNumber: 1,
        title: 'Ashes retained locally',
        datePlayed: '2026-09-09',
        notes: 'Exact notes retained after deletion.',
      },
    ]);
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Retry' }));

    const discard = screen.getByRole('button', { name: 'Discard changes' });
    discard.focus();
    await user.keyboard('{Enter}');
    await waitFor(() =>
      expect(
        screen.queryByRole('button', { name: /Ashes retained locally/ }),
      ).not.toBeInTheDocument(),
    );
    expect(coordinator.get(missingScope)).toBeUndefined();
    expect(coordinator.get<string>(unrelatedScope)?.value).toBe('Unsent unrelated Oracle question');
    expect(screen.getByRole('button', { name: /New session/i })).toHaveFocus();
  });
});
