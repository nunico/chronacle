import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import SessionLogView from './SessionLogView.svelte';
import type { Session } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import { sessionScope } from '../lib/drafts/draft-state';

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
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
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

describe('SessionLogView draft coordination', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(commands.getEntities).mockResolvedValue([]);
    vi.mocked(commands.getSessionEntities).mockResolvedValue([]);
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
});
