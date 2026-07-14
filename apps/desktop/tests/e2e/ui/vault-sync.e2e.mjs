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

  // Previously BLOCKED: there was no IPC-reachable way to set `vault_deleted`
  // (`delete_entity` hard-deletes). The inbound-sync tranche added
  // `soft_delete_entity`, so this scenario is now reachable through the same
  // invoke() path the UI uses.
  it('A soft-deleted record is not exported', async () => {
    // Given a campaign with two entities, synced to a vault
    const campaign = await invoke(driver, 'create_campaign', {
      name: 'Soft Delete Campaign',
      system: '5e',
    });
    const doomed = await invoke(driver, 'create_entity', {
      campaignId: campaign.id,
      kind: 'npc',
      input: entityInput('Doomed Informant'),
    });
    await invoke(driver, 'create_entity', {
      campaignId: campaign.id,
      kind: 'npc',
      input: entityInput('Surviving Informant'),
    });
    await invoke(driver, 'set_vault_path', { vaultPath: vaultDir });

    const dir = path.join(vaultDir, 'campaigns', 'soft-delete-campaign', 'entities', 'npc');
    assert.ok(
      fs.existsSync(path.join(dir, 'doomed-informant.md')),
      'the entity should have been exported before it is soft-deleted',
    );

    // When the GM deletes it (soft delete — the vault round-trip's delete)
    await invoke(driver, 'soft_delete_entity', { id: doomed.id, kind: 'npc' });
    await invoke(driver, 'vault_sync_now');

    // Then its vault file is gone, and its neighbour is untouched
    assert.ok(
      !fs.existsSync(path.join(dir, 'doomed-informant.md')),
      'a soft-deleted record must not remain in the vault',
    );
    assert.ok(
      fs.existsSync(path.join(dir, 'surviving-informant.md')),
      'soft-deleting one record must not disturb another',
    );
  });

  // Previously BLOCKED: `create_entity` hardcoded a required `campaign_id` and
  // always passed `collection_id: None`, so a collection-scoped entity could not
  // be created over IPC. The inbound-sync tranche made both scope arguments
  // optional (exactly one is required), so this is now reachable.
  it("A shared collection's entities are written once", async () => {
    // Given an entity scoped to a COLLECTION rather than a campaign
    const collection = await invoke(driver, 'create_collection', {
      name: 'Shared Bestiary',
      description: null,
    });
    await invoke(driver, 'create_entity', {
      collectionId: collection.id,
      kind: 'creature',
      input: entityInput('Sand Kraken'),
    });
    await invoke(driver, 'set_vault_path', { vaultPath: vaultDir });

    // Then it is written once, under collections/<slug>/ — not under any campaign
    const file = path.join(
      vaultDir,
      'collections',
      'shared-bestiary',
      'entities',
      'creature',
      'sand-kraken.md',
    );
    assert.ok(
      fs.existsSync(file),
      'a collection-scoped entity belongs under collections/<slug>/entities/<kind>/',
    );
    assert.match(
      fs.readFileSync(file, 'utf8'),
      /collection:\s*"Shared Bestiary"/,
      'frontmatter should record the owning collection',
    );

    const campaignCopies = fs.existsSync(path.join(vaultDir, 'campaigns'))
      ? fs
          .readdirSync(path.join(vaultDir, 'campaigns'), { recursive: true })
          .filter((p) => String(p).endsWith('sand-kraken.md'))
      : [];
    assert.equal(
      campaignCopies.length,
      0,
      'a collection-scoped entity must not be duplicated under any campaign',
    );
  });
});
