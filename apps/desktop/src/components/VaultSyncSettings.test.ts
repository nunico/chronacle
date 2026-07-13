import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import VaultSyncSettings from './VaultSyncSettings.svelte';
import * as commands from '../lib/commands';

describe('VaultSyncSettings', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(commands, 'listVaultConflicts').mockResolvedValue([]);
  });

  it('shows "not configured" when no vault path is set', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue(null);
    render(VaultSyncSettings);
    expect(await screen.findByText(/no vault configured/i)).toBeInTheDocument();
  });

  it('shows the configured path and enables Sync now', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    render(VaultSyncSettings);
    expect(await screen.findByText('/Users/gm/Vault')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sync now/i })).toBeEnabled();
  });

  it('disables Sync now when no vault is configured', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue(null);
    render(VaultSyncSettings);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /sync now/i })).toBeDisabled(),
    );
  });

  it('reports the export count after a successful sync', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    vi.spyOn(commands, 'vaultSyncNow').mockResolvedValue({
      exported: 3,
      unchanged: 7,
      adopted: 0,
      applied: 0,
      conflicts: 0,
      resolved: 0,
      soft_deleted: 0,
      swept: 0,
      invalid: 0,
      failed: 0,
    });
    render(VaultSyncSettings);
    await userEvent.click(await screen.findByRole('button', { name: /sync now/i }));
    expect(await screen.findByText(/3 exported/i)).toBeInTheDocument();
    expect(await screen.findByText(/7 unchanged/i)).toBeInTheDocument();
  });

  it('surfaces a failure count rather than claiming success', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    vi.spyOn(commands, 'vaultSyncNow').mockResolvedValue({
      exported: 1,
      unchanged: 0,
      adopted: 0,
      applied: 0,
      conflicts: 0,
      resolved: 0,
      soft_deleted: 0,
      swept: 0,
      invalid: 0,
      failed: 2,
    });
    render(VaultSyncSettings);
    await userEvent.click(await screen.findByRole('button', { name: /sync now/i }));
    expect(await screen.findByText(/2 failed/i)).toBeInTheDocument();
  });

  it('clears the vault path when Disconnect is clicked', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    const setPath = vi.spyOn(commands, 'setVaultPath').mockResolvedValue();
    render(VaultSyncSettings);
    await userEvent.click(await screen.findByRole('button', { name: /disconnect/i }));
    expect(setPath).toHaveBeenCalledWith(null);
  });

  it('lists each conflicted record with its resolution hint', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    vi.spyOn(commands, 'listVaultConflicts').mockResolvedValue([
      {
        id: 'n1',
        kind: 'npc',
        name: 'Seraphina Aldric',
        key: 'campaigns/sov/entities/npc/seraphina-aldric.md',
        sidecarKey: 'campaigns/sov/entities/npc/seraphina-aldric.conflict.md',
      },
    ]);
    render(VaultSyncSettings);
    // The GM must be able to identify the record AND find both files, so every
    // field of the row is asserted — a count alone is not enough.
    expect(await screen.findByText('Seraphina Aldric')).toBeInTheDocument();
    expect(screen.getByText('npc')).toBeInTheDocument();
    expect(screen.getByText('campaigns/sov/entities/npc/seraphina-aldric.md')).toBeInTheDocument();
    expect(
      screen.getByText('campaigns/sov/entities/npc/seraphina-aldric.conflict.md'),
    ).toBeInTheDocument();
    expect(screen.getByText(/\(1\)/)).toBeInTheDocument();
    expect(screen.getByText(/delete the \.conflict\.md file/i)).toBeInTheDocument();
  });

  it('shows no conflict section when there are none', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    vi.spyOn(commands, 'listVaultConflicts').mockResolvedValue([]);
    render(VaultSyncSettings);
    await screen.findByText('/Users/gm/Vault');
    expect(screen.queryByText(/conflict/i)).not.toBeInTheDocument();
  });

  /// A sync is exactly when a conflict appears or gets resolved, so a stale
  /// list after "Sync now" would show the GM a conflict they just fixed.
  it('reloads the conflict list after a sync', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    vi.spyOn(commands, 'vaultSyncNow').mockResolvedValue({
      exported: 0,
      unchanged: 0,
      adopted: 0,
      applied: 0,
      conflicts: 1,
      resolved: 0,
      soft_deleted: 0,
      swept: 0,
      invalid: 0,
      failed: 0,
    });
    const listConflicts = vi
      .spyOn(commands, 'listVaultConflicts')
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        {
          id: 'n1',
          kind: 'npc',
          name: 'Seraphina Aldric',
          key: 'campaigns/sov/entities/npc/seraphina-aldric.md',
          sidecarKey: 'campaigns/sov/entities/npc/seraphina-aldric.conflict.md',
        },
      ]);

    render(VaultSyncSettings);
    await screen.findByText('/Users/gm/Vault');
    expect(screen.queryByText('Seraphina Aldric')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /sync now/i }));

    // The conflict raised by the sync must show up without a reload.
    expect(await screen.findByText('Seraphina Aldric')).toBeInTheDocument();
    expect(listConflicts.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('survives a failure to load the conflict list', async () => {
    vi.spyOn(commands, 'getVaultPath').mockResolvedValue('/Users/gm/Vault');
    vi.spyOn(commands, 'listVaultConflicts').mockRejectedValue(new Error('backend down'));
    render(VaultSyncSettings);
    // The panel still renders; the supplementary list simply stays empty.
    expect(await screen.findByText('/Users/gm/Vault')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sync now/i })).toBeEnabled();
  });
});
