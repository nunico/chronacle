---
translationKey: troubleshooting.common
locale: en
slug: troubleshooting/common-problems
title: Common problems
navTitle: Common problems
summary: Use visible status and exact errors to recover safely from setup, provider, source, campaign, Codex, search, and vault problems.
section: troubleshooting
order: 1
headings:
  - id: model-download-fails
    text: The first model download fails
    level: 2
  - id: provider-does-not-work
    text: The answer provider does not work
    level: 2
  - id: pdf-import-fails
    text: A PDF does not import
    level: 2
  - id: indexing-needs-attention
    text: Indexing needs attention
    level: 2
  - id: no-results
    text: Search returns no results
    level: 2
  - id: search-unavailable
    text: Search is unavailable
    level: 2
  - id: campaign-content-is-missing
    text: Campaign content is missing
    level: 2
  - id: codex-article-does-not-change
    text: A Codex Article does not change
    level: 2
  - id: vault-does-not-settle
    text: The Markdown vault does not settle
    level: 2
---

Start with the exact status or error you can see. A message identifies the failed step; unless it says more, it does not prove the underlying cause.

<h2 id="model-download-fails">The first model download fails</h2>

**Symptom.** Setup shows **Download failed**, or the selected local search model is still not ready.

**Likely cause.** Chronacle could not finish or confirm that model download. The message alone does not say whether the interruption came from connectivity, storage, or another local condition.

**Safe checks.** Keep the app open, copy any detailed error, confirm you have free disk space and an ordinary network connection, and check whether **Retry** is available. Do not move or edit Chronacle’s data folders.

**Recovery.** Choose **Retry**. For a later model choice, use **Settings → Embedding provider → Download selected model**, then save that provider again. If downloads keep failing, preserve the exact error and use [Choose an AI provider](/en/manual/ai-providers/choose) to select another supported search setup.

<h2 id="provider-does-not-work">The answer provider does not work</h2>

**Symptom.** Settings shows `Connection failed: {error}`, or a question returns a provider error after Settings showed `Connected: {provider}`.

**Likely cause.** `Failed to save: {error}` means the edited fields were not all stored. `Connection failed: {error}` means activation failed. `Connected: {provider}` only confirms that a provider was activated from stored values; it does not prove that a preceding failed save took effect or that a later service request will succeed.

**Safe checks.** Check whether a save error appeared before `Connected: {provider}`. Confirm the selected Provider, exact Model, and Base URL. OpenAI, Anthropic, and custom providers require an API key in the connection form; Ollama does not. Never paste the key into chat or support text.

**Recovery.** Correct one visible field at a time and use **Save & Connect** again. For Ollama, also confirm the local service and chosen model are available. Follow [Set up an online provider](/en/manual/ai-providers/online), [Use a local provider](/en/manual/ai-providers/local), or [Set up a custom provider](/en/manual/ai-providers/custom).

<h2 id="pdf-import-fails">A PDF does not import</h2>

**Symptom.** Chronacle shows `"{name}" failed to upload: {error}` or `"{name}" failed to index: {error}`.

**Likely cause.** The first message means the file could not be accepted or stored; the second means processing failed after upload. The appended `{error}` is required to narrow it further.

**Safe checks.** Confirm you selected a PDF you are allowed to use, that it still opens normally, and that no other upload is already in progress. Record the filename and full appended error without sharing the PDF itself.

**Recovery.** Retry that file once through **Upload PDF**. If upload succeeds but indexing fails again, keep the source’s error status and exact message; do not repeatedly delete unrelated collections. See [Import PDFs](/en/manual/source-library/upload-pdfs).

<h2 id="indexing-needs-attention">Indexing needs attention</h2>

**Symptom.** A banner says sources were indexed with a different model, or Settings shows `Re-index failed: {error}` / `Re-indexing failed: {error}`.

**Likely cause.** A model-mismatch banner means the active search model differs from the one recorded on those sources. A failure message only identifies the unsuccessful re-index step; its appended error may say more.

**Safe checks.** In **Settings → Embedding provider**, confirm the intended mode and active model. Check that a selected local model is downloaded or that cloud fields are complete. Chronacle deletes the current source’s old passages before rebuilding it, with no rollback, so that source is unavailable throughout the attempt.

**Recovery.** Use **Re-index now** in the banner or **Re-index all sources** in Settings, and wait for the count to finish. If it fails, that source remains unavailable in search until a retry succeeds. Preserve the exact error, correct only the indicated provider problem, and retry. Other sources remain available. See [Understand indexing](/en/manual/source-library/indexing).

<h2 id="no-results">Search returns no results</h2>

**Symptom.** A manual search finishes without finding an article.

**Likely cause.** The wording may not occur in the index, or the relevant article may be in the other manual language. Manual search uses only the language of the page you are reading.

**Safe checks.** Confirm the manual language, shorten the query, and try the exact name of a visible control or feature.

**Recovery.** Open the manual overview or browse through section navigation. Switch to the other manual language before searching translated terms.

<h2 id="search-unavailable">Search is unavailable</h2>

**Symptom.** The manual search dialog says that search is unavailable.

**Likely cause.** The static search files did not load. That message does not identify a more specific cause.

**Safe checks.** Reload the page once and confirm that the manual itself still opens normally.

**Recovery.** Continue through the manual overview or section navigation. Search is optional and does not block direct navigation.

<h2 id="campaign-content-is-missing">Campaign content is missing</h2>

**Symptom.** **No campaign** is shown, campaign pages ask you to select one, or an answer does not use an expected collection.

**Likely cause.** No campaign is active, or the active campaign is not subscribed to that source collection. A collection can exist without being available to every campaign.

**Safe checks.** Confirm the active campaign in the campaign rail. Open **Campaign & sources** and inspect whether the expected collection is marked **subscribed** and its source is **Indexed** rather than **Indexing…** or **Error**.

**Recovery.** Select or create the intended campaign, subscribe the collection, and wait for indexing to complete before asking again. See [Control campaign source access](/en/manual/campaigns/source-access).

<h2 id="codex-article-does-not-change">A Codex Article does not change</h2>

**Symptom.** Recompile shows `No source context found — article unchanged`, **Failed to recompile article**, or a collection compile leaves an item stale.

**Likely cause.** The first message means Chronacle found no usable source passage for that entity and deliberately kept the existing article. Other failures need their displayed detail or the collection status; do not assume missing material is the cause.

**Safe checks.** Confirm the relevant collection is indexed and available to the entity’s campaign or collection. Check the entity name and alternate names, and keep your Summary and Notes unchanged while investigating.

**Recovery.** Correct source access or identity, then recompile the single article or collection. Do not copy generated prose into Notes merely to force compilation; Notes are your lasting record. See [Compile the Codex](/en/manual/codex/compile) and [Maintain the Codex](/en/manual/codex/maintenance).

<h2 id="vault-does-not-settle">The Markdown vault does not settle</h2>

**Symptom.** The panel shows `Sync failed: {error}`, reports invalid or failed files, or keeps listing a conflict.

**Likely cause.** `Sync failed` means the full folder check failed; an invalid count means at least one managed file could not be parsed; a listed conflict means both versions changed. None of these alone identifies which external action caused the state.

**Safe checks.** Copy the exact error and listed paths, make a backup, confirm the selected folder still exists and is a directory, and inspect managed files for intact `---` metadata and `id`. For a conflict, compare the normal file and `.conflict.md` sidecar.

**Recovery.** Repair invalid metadata from a backup, then choose **Sync now**. Resolve a conflict only with the procedure in [Resolve vault conflicts](/en/manual/vault/conflicts). If a folder switch failed, the previous folder remains active; follow [Switch the vault folder](/en/manual/vault/switch-folder) rather than deleting files.
