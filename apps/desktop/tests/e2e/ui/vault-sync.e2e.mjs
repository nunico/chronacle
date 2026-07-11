// End-to-end: Markdown vault sync — export, no-op resync, soft-delete
// exclusion, and shared-collection dedup.
//
// Drives the REAL built app over tauri-driver: real Rust backend, real
// SurrealDB, real chronacle-vault reconcile/export. Setup uses `invoke()`
// through the live webview IPC — the same path the UI uses — to seed a
// campaign, an entity, and a collection. `set_vault_path` / `vault_sync_now`
// are invoked the same way the VaultSyncSettings component calls them; the
// resulting Markdown files are then read straight off disk with Node `fs` and
// asserted against.
//
// NOTE: this suite is Linux/Windows-only (tauri-driver has no macOS support)
// and requires a built release binary — see README.md. It cannot be run from
// this dev machine; it runs in Linux CI on merge-to-main.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  startTauriDriver,
  buildDriver,
  invoke,
  waitForWebviewReady,
} from './driver.mjs';

// EntityInput (crates/chronacle-extraction/src/entity_service/types.rs) has
// no #[serde(default)] on its optional fields, so every key must be present —
// mirrors the full object EntityForm.svelte always sends (see
// src/components/EntityForm.svelte's handleSubmit).
function entityInput(name, summary = null) {
  return {
    name,
    summary,
    notes: null,
    dateStart: null,
    dateEnd: null,
    isOngoing: null,
    sequenceIndex: null,
    era: null,
    durationLabel: null,
    sessionId: null,
    playerName: null,
    characterClass: null,
    characterLevel: null,
    status: null,
  };
}

describe('Markdown vault sync — export', function () {
  this.timeout(120000);

  let tauriDriver;
  let driver;
  let vaultDir;

  before(async () => {
    tauriDriver = startTauriDriver();
    driver = await buildDriver();
    await waitForWebviewReady(driver);
  });

  after(async () => {
    // Turn vault sync back off so later suites in the same run start clean.
    if (driver) {
      try {
        await invoke(driver, 'set_vault_path', { vaultPath: null });
      } catch {
        // best effort
      }
      await driver.quit();
    }
    if (tauriDriver) tauriDriver.kill();
  });

  beforeEach(() => {
    vaultDir = fs.mkdtempSync(path.join(os.tmpdir(), 'chronacle-vault-e2e-'));
  });

  it('Configuring a vault exports every record', async () => {
    // Given a campaign "Shadows of Valdris" with an entity "Seraphina Aldric"
    const campaign = await invoke(driver, 'create_campaign', {
      name: 'Shadows of Valdris',
      system: '5e',
    });
    await invoke(driver, 'create_entity', {
      campaignId: campaign.id,
      kind: 'npc',
      input: entityInput('Seraphina Aldric', 'A wandering oracle.'),
    });

    // When the GM sets the vault path to a temporary directory
    await invoke(driver, 'set_vault_path', { vaultPath: vaultDir });

    // Then a file exists at "campaigns/shadows-of-valdris/entities/npc/seraphina-aldric.md"
    const filePath = path.join(
      vaultDir,
      'campaigns',
      'shadows-of-valdris',
      'entities',
      'npc',
      'seraphina-aldric.md',
    );
    assert.ok(fs.existsSync(filePath), `expected exported file at ${filePath}`);

    // And that file's frontmatter carries the alias "Seraphina Aldric"
    const content = fs.readFileSync(filePath, 'utf8');
    assert.match(
      content,
      /aliases:\s*\[[^\]]*"Seraphina Aldric"[^\]]*\]/,
      'frontmatter should carry the "Seraphina Aldric" alias',
    );
  });

  it('Syncing again writes nothing', async () => {
    // Given a campaign with a configured vault that has been synced
    const campaign = await invoke(driver, 'create_campaign', {
      name: 'Second Sync Campaign',
      system: '5e',
    });
    await invoke(driver, 'create_entity', {
      campaignId: campaign.id,
      kind: 'npc',
      input: entityInput('Idle NPC'),
    });
    await invoke(driver, 'set_vault_path', { vaultPath: vaultDir });

    // When the GM clicks "Sync now"
    const report = await invoke(driver, 'vault_sync_now');

    // Then the reconcile report shows 0 exported
    assert.equal(report.exported, 0, `expected a no-op resync, got ${JSON.stringify(report)}`);
  });

  // A soft-deleted record (vault_deleted = true) is not exported. There is no
  // Tauri command that sets vault_deleted — delete_entity hard-deletes the
  // row (crates/chronacle-extraction/src/entity_service/crud/write.rs:228),
  // and no other command exposes the soft-delete flag. Seeding this scenario
  // through real invoke() calls is not possible without inventing a command
  // name, which the task brief explicitly forbids. BLOCKED — left unimplemented
  // pending a seeding command (or a documented direct-DB test hook) for
  // vault_deleted.
  it('A soft-deleted record is not exported (BLOCKED: no invoke-able seeding command for vault_deleted)');

  // A shared collection's entities are written once under collections/<slug>/
  // when an entity is scoped to a *collection* (an `in_collection` graph edge)
  // rather than a campaign — see VaultScope::Collection in
  // crates/chronacle-vault/src/keys.rs and the scope resolution in
  // crates/chronacle-domain/src/vault_record_store.rs:118-125.
  // entity_service::create() supports this (it takes an optional
  // collection_id and RELATEs via in_collection — write.rs:83), but the
  // exposed `create_entity` Tauri command
  // (apps/desktop/src-tauri/src/commands/entity_commands.rs:90-100) hardcodes
  // `campaign_id: String` as required and always passes `collection_id: None`
  // to the service call — there is no IPC-reachable way to create a
  // collection-scoped entity. Seeding this scenario would require inventing a
  // command (or an argument shape) that isn't there, which the task brief
  // explicitly forbids. BLOCKED — left unimplemented pending a seeding
  // command that can attach an entity to a collection instead of a campaign.
  it(
    "A shared collection's entities are written once " +
      '(BLOCKED: create_entity has no IPC path to collection-scope an entity)',
  );
});
