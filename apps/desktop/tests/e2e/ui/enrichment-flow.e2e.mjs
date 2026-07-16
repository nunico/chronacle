// End-to-end: the opt-in second-pass enrichment of related-entity summaries.
//
// Drives the REAL built app over tauri-driver: real Rust backend, real
// SurrealDB, real chunking + embeddings, real PDF extraction. Only the LLM is
// swapped for a deterministic local stub (so the assertion is reproducible and
// no API key is needed). Setup uses `invoke()` through the live webview IPC —
// the same path the UI uses — to avoid native file dialogs.
//
// Flow: configure stub LLM + enable enrichment → index a lore PDF whose text
// names a seed ("Commander Varn") and a related faction ("The Iron Fist") →
// extract the seed → assert the faction's summary was rewritten from the
// RELATIONAL first-pass text into the ENTITY-CENTRIC profile text.
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { startStubLlm, PROFILE_SUMMARY, RELATIONAL_SUMMARY } from './stub-llm.mjs';
import {
  startTauriDriver,
  buildDriver,
  invoke,
  pollUntil,
  waitForWebviewReady,
} from './driver.mjs';

const FIXTURE_PDF = fileURLToPath(new URL('./fixtures/lore-iron-fist.pdf', import.meta.url));

describe('entity extraction — neighbor enrichment second pass', function () {
  this.timeout(180000); // model download + indexing + two LLM passes

  let stub;
  let tauriDriver;
  let driver;

  before(async () => {
    stub = await startStubLlm();
    tauriDriver = startTauriDriver();
    driver = await buildDriver();
    await waitForWebviewReady(driver);
  });

  after(async () => {
    if (driver) await driver.quit();
    if (tauriDriver) tauriDriver.kill();
    if (stub) await stub.close();
  });

  it('rewrites a related entity summary to be entity-centric', async () => {
    // 1. Point the LLM at the deterministic stub and switch enrichment ON.
    await invoke(driver, 'update_setting', { key: 'llm_provider', value: 'openai' });
    await invoke(driver, 'update_setting', { key: 'llm_base_url', value: stub.url });
    await invoke(driver, 'update_setting', { key: 'llm_api_key', value: 'stub-key' });
    await invoke(driver, 'update_setting', { key: 'llm_model', value: 'stub-model' });
    await invoke(driver, 'update_setting', {
      key: 'extraction_enrich_neighbors',
      value: 'true',
    });
    await invoke(driver, 'reconfigure_llm_provider');

    // Force the LOCAL embedding backend. This spec exercises ingestion +
    // extraction + enrichment, not embedding quality, so the deterministic
    // mock embedder is exactly right — and it needs no API key. We must set
    // this explicitly rather than lean on the default: the default resolves to
    // `local` only where an ONNX Runtime library is present, and the
    // `--no-bundle` Linux CI build has neither a bundled nor a system ORT, so
    // the default silently flips to `openai` and ingestion fails with
    // "OpenAI embedding API key is not configured". With `local` and no cached
    // model, `build_embedding_provider_from_map` returns the mock embedder.
    await invoke(driver, 'update_setting', { key: 'embedding_backend', value: 'local' });
    await invoke(driver, 'reconfigure_embedding_provider');

    // 2. Index the lore PDF into a fresh collection (real ingestion pipeline;
    //    upload_source blocks until chunks are embedded).
    const collection = await invoke(driver, 'create_collection', {
      name: 'E2E Lore',
      description: null,
    });
    await invoke(driver, 'upload_source', {
      filePath: FIXTURE_PDF,
      displayName: 'Lore',
      sourceType: 'lore',
      collectionId: collection.id,
    });

    // 3. Attach the collection to a campaign.
    const campaign = await invoke(driver, 'create_campaign', {
      name: 'E2E Campaign',
      system: '5e',
    });
    await invoke(driver, 'add_campaign_collection', {
      campaignId: campaign.id,
      collectionId: collection.id,
    });

    // 4. Seed-anchored extraction of the NPC. Blocks through the second pass.
    const summary = await invoke(driver, 'extract_entity_by_name', {
      campaignId: campaign.id,
      name: 'Commander Varn',
    });
    assert.ok(
      summary.entities_created >= 2,
      `expected seed + neighbor created, got ${JSON.stringify(summary)}`,
    );

    // 5. The faction neighbor must exist with the ENTITY-CENTRIC summary from
    //    the profile pass — not the relational first-pass summary.
    const fist = await pollUntil(
      async () => {
        const factions = await invoke(driver, 'get_entities', {
          campaignId: campaign.id,
          kind: 'faction',
        });
        return factions.find((f) => f.name === 'The Iron Fist');
      },
      { timeoutMs: 30000, intervalMs: 1000 },
    );

    assert.equal(
      fist.summary,
      PROFILE_SUMMARY,
      'enrichment should overwrite the summary with the entity-centric profile',
    );
    assert.notEqual(
      fist.summary,
      RELATIONAL_SUMMARY,
      'the relational first-pass summary must not survive',
    );
    assert.ok(
      stub.calls.includes('profile'),
      `the second (profile) pass should have hit the LLM; calls=${stub.calls}`,
    );
  });
});
