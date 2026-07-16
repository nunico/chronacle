# Maintenance Naming-Conflict Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Maintenance findings list scroll, and replace the raw-record-ID naming-conflict card with a name-based card that lets a GM resolve the conflict (assign the disputed term to one entity, merge, open, or dismiss).

**Architecture:** Backend enriches `alias_collision` / `duplicate_entity` findings with real entity names + a "term is this entity's primary name" flag at read time (`normalize` stays in Rust). A new `resolve_alias_collision` command strips the disputed alias from the losing entity, re-validating server-side that it is an alias and never a primary name. The Svelte card renders names and wires up the four actions.

**Tech Stack:** Rust (chronacle-extraction codex/lint service, Tauri commands), Svelte 5 runes (`apps/desktop/src`), Vitest, playwright-bdd.

**Spec:** `docs/superpowers/specs/2026-07-16-maintenance-naming-conflict-design.md`

---

## File Structure

- **Modify** `crates/chronacle-extraction/src/codex_service/lint.rs` — add `Identity` struct, `lookup_identity`, `enrich_finding_display` (called from `list_lint_findings`), and `resolve_alias_collision`.
- **Modify** `crates/chronacle-extraction/src/codex_service/mod.rs` — re-export `resolve_alias_collision`.
- **Modify** `apps/desktop/src-tauri/src/commands/codex_commands.rs` — add `resolve_alias_collision` Tauri command + smoke-test reference.
- **Modify** `apps/desktop/src-tauri/src/lib.rs` — register the command in the invoke handler.
- **Modify** `apps/desktop/src/lib/commands.ts` — add `resolveAliasCollision` wrapper.
- **Modify** `apps/desktop/src/views/MaintenanceView.svelte` — scroll CSS; rewrite the `alias_collision` card; name display on the `duplicate_entity` card; `resolveCollision` handler.
- **Modify** `apps/desktop/src/views/MaintenanceView.test.ts` — scroll guard + naming-conflict card tests.
- **Modify** `apps/desktop/tests/e2e/features/maintenance-inbox.feature` — resolution scenario.
- **Modify** `apps/desktop/tests/e2e/backend/steps/maintenance.steps.ts` — step defs for the scenario.

---

## Task 1: Backend — enrich findings with real names

**Files:**
- Modify: `crates/chronacle-extraction/src/codex_service/lint.rs` (add helpers; call from `list_lint_findings` at lines 637-645)
- Test: `crates/chronacle-extraction/src/codex_service/lint_tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/chronacle-extraction/src/codex_service/lint_tests.rs`:

```rust
/// `list_lint_findings` must inject real names and a name-vs-alias flag so the
/// Maintenance UI never shows raw record ids.
#[tokio::test]
async fn list_findings_enriches_alias_collision_with_names_and_flags() {
    use crate::codex_service::list_lint_findings;
    let db = setup_db().await;
    seed_campaign(&db).await;
    // `a`'s NAME normalizes to the key; `b` holds it only as an alias.
    db.query(
        "CREATE npc:`a` SET name='Consortium', summary='The old guild', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`b` SET name='Trade Guild', aliases=['Consortium'], summary=NULL, \
             notes=NULL, created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`a` SET created_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`b` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    run_lint_campaign(&db, "camp1").await.unwrap();

    let findings = list_lint_findings(&db).await.unwrap();
    let f = findings
        .iter()
        .find(|f| f.kind == "alias_collision")
        .expect("collision finding present");

    // Names present, ordered by whichever side is `a`/`b` in the payload.
    let a_id = f.payload.get("a").and_then(|v| v.as_str()).unwrap();
    let (a_name, a_is_name, b_name, b_is_name) = if a_id == "npc:a" {
        ("Consortium", true, "Trade Guild", false)
    } else {
        ("Trade Guild", false, "Consortium", true)
    };
    assert_eq!(f.payload.get("a_name").and_then(|v| v.as_str()), Some(a_name));
    assert_eq!(f.payload.get("b_name").and_then(|v| v.as_str()), Some(b_name));
    assert_eq!(f.payload.get("a_is_name").and_then(|v| v.as_bool()), Some(a_is_name));
    assert_eq!(f.payload.get("b_is_name").and_then(|v| v.as_bool()), Some(b_is_name));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p chronacle-extraction list_findings_enriches_alias_collision -- --nocapture`
Expected: FAIL — `a_name` etc. are absent (assertion `Some("Consortium")` vs `None`).

- [ ] **Step 3: Add the enrichment helpers**

In `crates/chronacle-extraction/src/codex_service/lint.rs`, add above `list_lint_findings` (near line 616):

```rust
/// Display identity for a finding party, looked up fresh at read time.
struct Identity {
    name: String,
    aliases: Vec<String>,
    summary: Option<String>,
}

/// Look up name/aliases/summary for a full record id (`kind:id`), skipping
/// soft-deleted rows (`vault_deleted = true`). Returns `None` if the record is
/// missing or deleted — the caller falls back to showing the raw id.
async fn lookup_identity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    full_id: &str,
) -> Result<Option<Identity>, String> {
    let Some((table, id)) = full_id.split_once(':') else {
        return Ok(None);
    };
    #[derive(Deserialize)]
    struct Row {
        name: String,
        #[serde(default)]
        aliases: Vec<String>,
        #[serde(default)]
        summary: Option<String>,
    }
    let mut resp = db
        .query(
            "SELECT name, aliases, summary FROM type::thing($tb, $id) \
             WHERE vault_deleted != true",
        )
        .bind(("tb", table.to_owned()))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to look up entity: {e}"))?;
    let rows: Vec<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse entity identity: {e}"))?;
    Ok(rows.into_iter().next().map(|r| Identity {
        name: r.name,
        aliases: r.aliases,
        summary: r.summary,
    }))
}

/// Attach human-readable names (and, for alias collisions, a `*_is_name` flag)
/// to a finding so the Maintenance UI can render conflicts without exposing raw
/// record ids. Other kinds pass through untouched.
async fn enrich_finding_display<C: Connection>(
    db: &surrealdb::Surreal<C>,
    finding: &mut LintFinding,
) -> Result<(), String> {
    if finding.kind != "alias_collision" && finding.kind != "duplicate_entity" {
        return Ok(());
    }
    let key = finding
        .payload
        .get("alias")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let Some(obj) = finding.payload.as_object_mut() else {
        return Ok(());
    };
    for side in ["a", "b"] {
        let Some(full_id) = obj.get(side).and_then(|v| v.as_str()).map(str::to_owned) else {
            continue;
        };
        if let Some(identity) = lookup_identity(db, &full_id).await? {
            obj.insert(format!("{side}_name"), json!(identity.name.clone()));
            obj.insert(format!("{side}_summary"), json!(identity.summary));
            // Only alias collisions carry a normalized key to compare against.
            if let Some(k) = key.as_deref() {
                let is_name = naming::normalize(&identity.name) == k;
                obj.insert(format!("{side}_is_name"), json!(is_name));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Call the enrichment from `list_lint_findings`**

In `list_lint_findings` (lint.rs ~637-645), replace the final `Ok(rows.into_iter()...collect())` with:

```rust
    let mut findings: Vec<LintFinding> = rows
        .into_iter()
        .map(|r| LintFinding {
            id: r.id.id.to_raw(),
            kind: r.kind,
            payload: r.payload,
            created_at: r.created_at.to_string(),
        })
        .collect();
    for finding in &mut findings {
        enrich_finding_display(db, finding).await?;
    }
    Ok(findings)
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p chronacle-extraction list_findings_enriches_alias_collision -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-extraction/src/codex_service/lint.rs \
        crates/chronacle-extraction/src/codex_service/lint_tests.rs
git commit -m "feat(lint): enrich conflict findings with real names"
```

---

## Task 2: Backend — `resolve_alias_collision` service function

**Files:**
- Modify: `crates/chronacle-extraction/src/codex_service/lint.rs`
- Modify: `crates/chronacle-extraction/src/codex_service/mod.rs` (re-export)
- Test: `crates/chronacle-extraction/src/codex_service/lint_tests.rs`

- [ ] **Step 1: Write the failing tests**

Append to `lint_tests.rs`:

```rust
/// Assigning the term to `keep` strips it from `drop` and resolves the finding.
#[tokio::test]
async fn resolve_alias_collision_strips_the_alias_from_the_loser() {
    use crate::codex_service::{list_lint_findings, resolve_alias_collision};
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`a` SET name='Consortium', summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`b` SET name='Trade Guild', aliases=['Consortium'], summary=NULL, \
             notes=NULL, created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`a` SET created_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`b` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    run_lint_campaign(&db, "camp1").await.unwrap();
    let f = list_lint_findings(&db).await.unwrap();
    let f = f.iter().find(|f| f.kind == "alias_collision").unwrap();

    // `b` holds the term as an alias, so it is the valid drop target.
    resolve_alias_collision(&db, &f.id, "npc:a", "npc:b").await.unwrap();

    // Alias gone from b, and the finding is resolved (no longer listed).
    #[derive(serde::Deserialize)]
    struct Row { aliases: Vec<String> }
    let mut resp = db.query("SELECT aliases FROM npc:`b`").await.unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0].aliases.is_empty(), "alias must be removed from the loser");
    let remaining = list_lint_findings(&db).await.unwrap();
    assert!(remaining.iter().all(|x| x.kind != "alias_collision"));
}

/// The command must refuse to strip a term that is the loser's PRIMARY NAME —
/// a name cannot be removed. Nothing is mutated.
#[tokio::test]
async fn resolve_alias_collision_refuses_to_strip_a_primary_name() {
    use crate::codex_service::{list_lint_findings, resolve_alias_collision};
    let db = setup_db().await;
    seed_campaign(&db).await;
    // Both entities are literally NAMED the same normalized term.
    db.query(
        "CREATE npc:`a` SET name='Consortium', summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`b` SET name='Consortium', summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`a` SET created_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`b` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    run_lint_campaign(&db, "camp1").await.unwrap();
    let f = list_lint_findings(&db).await.unwrap();
    let f = f.iter().find(|f| f.kind == "alias_collision").unwrap();

    let err = resolve_alias_collision(&db, &f.id, "npc:a", "npc:b")
        .await
        .unwrap_err();
    assert!(err.contains("primary name"), "got: {err}");
    // Finding still open — nothing resolved.
    let remaining = list_lint_findings(&db).await.unwrap();
    assert!(remaining.iter().any(|x| x.kind == "alias_collision"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p chronacle-extraction resolve_alias_collision -- --nocapture`
Expected: FAIL — `resolve_alias_collision` is not defined.

- [ ] **Step 3: Implement `resolve_alias_collision`**

In `lint.rs`, add after `resolve_lint_finding` (~line 659):

```rust
/// Resolve a naming conflict by keeping the disputed term on `keep_id` and
/// stripping it from `drop_id`. `drop_id` must hold the term as an *alias*; if
/// it is that entity's primary name this errors and mutates nothing (a name
/// cannot be removed — the GM must merge or rename instead). `keep_id` is
/// validated to be the finding's other party but needs no mutation.
pub async fn resolve_alias_collision<C: Connection>(
    db: &surrealdb::Surreal<C>,
    finding_id: &str,
    keep_id: &str,
    drop_id: &str,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM type::thing('lint_finding', $id)")
        .bind(("id", finding_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to load finding: {e}"))?;
    let rows: Vec<Row> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse finding: {e}"))?;
    let payload = rows
        .into_iter()
        .next()
        .ok_or_else(|| "Finding not found".to_string())?
        .payload;

    let key = payload
        .get("alias")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Finding has no alias key".to_string())?;
    let a = payload.get("a").and_then(|v| v.as_str());
    let b = payload.get("b").and_then(|v| v.as_str());
    let valid = matches!(
        (a, b),
        (Some(x), Some(y))
            if (x == keep_id && y == drop_id) || (x == drop_id && y == keep_id)
    );
    if !valid {
        return Err("keep_id/drop_id do not match this finding".into());
    }

    let identity = lookup_identity(db, drop_id)
        .await?
        .ok_or_else(|| "Losing entity no longer exists".to_string())?;

    // Find the loser's original-cased alias whose normalized form is the key.
    let original = identity
        .aliases
        .iter()
        .find(|al| naming::normalize(al) == key);
    let Some(original) = original else {
        // Not an alias — it must be the primary name (or a stale finding).
        if naming::normalize(&identity.name) == key {
            return Err("Cannot strip a primary name; merge or rename instead".into());
        }
        return Err("Losing entity does not claim this term".into());
    };

    crate::entity_service::remove_alias(db, drop_id, original)
        .await
        .map_err(|e| e.to_string())?;
    resolve_lint_finding(db, finding_id).await
}
```

- [ ] **Step 4: Re-export from `mod.rs`**

In `crates/chronacle-extraction/src/codex_service/mod.rs`, extend the `pub use lint::{...}` block to include `resolve_alias_collision`:

```rust
pub use lint::{
    list_lint_findings, resolve_alias_collision, resolve_lint_finding, run_lint_campaign,
    run_lint_collection, LintFinding, LintSummary,
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p chronacle-extraction resolve_alias_collision -- --nocapture`
Expected: PASS (both tests)

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-extraction/src/codex_service/lint.rs \
        crates/chronacle-extraction/src/codex_service/mod.rs \
        crates/chronacle-extraction/src/codex_service/lint_tests.rs
git commit -m "feat(lint): resolve_alias_collision strips loser's alias"
```

---

## Task 3: Backend — Tauri command + registration

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/codex_commands.rs` (command + smoke test ~line 314)
- Modify: `apps/desktop/src-tauri/src/lib.rs` (invoke handler ~line 556)

- [ ] **Step 1: Add the command**

In `codex_commands.rs`, after `resolve_lint_finding` (~line 298):

```rust
/// Resolve a naming conflict: keep the disputed term on `keep_id` and strip it
/// from `drop_id` (whose claim must be an alias, not its primary name).
#[tauri::command]
pub async fn resolve_alias_collision(
    state: State<'_, Arc<AppState>>,
    finding_id: String,
    keep_id: String,
    drop_id: String,
) -> Result<(), String> {
    chronacle_extraction::codex_service::resolve_alias_collision(
        &state.db,
        &finding_id,
        &keep_id,
        &drop_id,
    )
    .await
}
```

- [ ] **Step 2: Reference it in the smoke test**

In the `proposal_commands_module_compiles` test (~line 315), add:

```rust
        let _ = resolve_alias_collision as fn(_, _, _, _) -> _;
```

- [ ] **Step 3: Register in the invoke handler**

In `apps/desktop/src-tauri/src/lib.rs`, after `commands::resolve_lint_finding,` (line 556):

```rust
            commands::resolve_alias_collision,
```

- [ ] **Step 4: Verify it compiles and the smoke test passes**

Run: `cargo test -p chronacle-desktop proposal_commands_module_compiles`
Expected: PASS (command wired, signature valid)

Note: if the desktop crate name differs, use `cargo test -p $(cargo metadata --no-deps --format-version 1 | python3 -c "import json,sys;print([p['name'] for p in json.load(sys.stdin)['packages'] if 'src-tauri' in p['manifest_path']][0])") proposal_commands_module_compiles`. The simplest fallback is `cargo build --workspace`.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/codex_commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(commands): expose resolve_alias_collision over IPC"
```

---

## Task 4: Frontend — `resolveAliasCollision` wrapper

**Files:**
- Modify: `apps/desktop/src/lib/commands.ts` (after `resolveLintFinding`, ~line 864)

- [ ] **Step 1: Add the wrapper**

```ts
/**
 * Resolve a naming conflict: `keepId` retains the disputed term, `dropId` has
 * it removed as an alias. Throws if `dropId` holds the term as its primary
 * name (a name cannot be stripped).
 */
export async function resolveAliasCollision(
  findingId: string,
  keepId: string,
  dropId: string,
): Promise<void> {
  return invoke('resolve_alias_collision', { findingId, keepId, dropId });
}
```

- [ ] **Step 2: Typecheck**

Run: `pnpm -C apps/desktop typecheck`
Expected: PASS (no errors)

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/lib/commands.ts
git commit -m "feat(commands): resolveAliasCollision invoke wrapper"
```

---

## Task 5: Frontend — scrollable Maintenance view + guard

**Files:**
- Modify: `apps/desktop/src/views/MaintenanceView.svelte` (`.maintenance` rule ~line 526)
- Test: `apps/desktop/src/views/MaintenanceView.test.ts`

- [ ] **Step 1: Write the failing guard test**

Add to `MaintenanceView.test.ts` (top-level, after imports):

```ts
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

describe('MaintenanceView scroll (regression: clipped findings)', () => {
  it('.maintenance root is its own scroll container', () => {
    const src = readFileSync(
      fileURLToPath(new URL('./MaintenanceView.svelte', import.meta.url)),
      'utf8',
    );
    const block = src.match(/\.maintenance\s*\{[^}]*\}/)?.[0] ?? '';
    expect(block).toMatch(/overflow-y:\s*auto/);
    expect(block).toMatch(/height:\s*100%/);
  });
});
```

> Rationale: `<main class="main">` in `Shell.svelte` is `overflow: hidden`; jsdom does not apply stylesheet cascade to `getComputedStyle`, so the invariant is guarded at the source level — anyone deleting the scroll rule fails this test.

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm -C apps/desktop exec vitest run src/views/MaintenanceView.test.ts -t "scroll container"`
Expected: FAIL — the `.maintenance` block has no `overflow-y`.

- [ ] **Step 3: Add the scroll CSS**

In `MaintenanceView.svelte`, change the `.maintenance` rule (line 526):

```css
  .maintenance {
    height: 100%;
    overflow-y: auto;
    box-sizing: border-box;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm -C apps/desktop exec vitest run src/views/MaintenanceView.test.ts -t "scroll container"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/views/MaintenanceView.svelte apps/desktop/src/views/MaintenanceView.test.ts
git commit -m "fix(maintenance): make findings list scrollable"
```

---

## Task 6: Frontend — naming-conflict card redesign

**Files:**
- Modify: `apps/desktop/src/views/MaintenanceView.svelte` (script handler; `alias_collision` branch ~460-490; `duplicate_entity` display ~429-431; styles)
- Test: `apps/desktop/src/views/MaintenanceView.test.ts`

- [ ] **Step 1: Write the failing tests**

In `MaintenanceView.test.ts`, first add `resolveAliasCollision` to the `vi.mock('../lib/commands', ...)` factory:

```ts
  resolveAliasCollision: vi.fn().mockResolvedValue(undefined),
```

Then add this suite:

```ts
describe('MaintenanceView naming-conflict card', () => {
  function collision(overrides = {}) {
    return finding({
      id: 'lint_finding:c1',
      kind: 'alias_collision',
      payload: {
        alias: 'consortium',
        a: 'faction:a',
        b: 'faction:b',
        a_name: 'Merchant Consortium',
        b_name: 'Trade Consortium',
        a_is_name: false,
        b_is_name: false,
        ...overrides,
      },
    });
  }

  it('renders entity names, not raw record ids', async () => {
    m.getLintFindings.mockResolvedValue([collision()]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    expect(await screen.findByText('Merchant Consortium')).toBeInTheDocument();
    expect(screen.getByText('Trade Consortium')).toBeInTheDocument();
    expect(screen.queryByText('faction:a')).not.toBeInTheDocument();
  });

  it('assigns the term to one entity and strips the other', async () => {
    m.getLintFindings.mockResolvedValue([collision()]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Keep on Merchant Consortium' }),
    );
    expect(m.resolveAliasCollision).toHaveBeenCalledWith(
      'lint_finding:c1',
      'faction:a',
      'faction:b',
    );
  });

  it('hides the Keep button on the side whose term is its primary name', async () => {
    // `a` holds the term as its name → cannot strip from `a` → no "Keep on Trade Consortium".
    m.getLintFindings.mockResolvedValue([collision({ a_is_name: true })]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await screen.findByText('Merchant Consortium');
    expect(
      screen.queryByRole('button', { name: 'Keep on Trade Consortium' }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Keep on Merchant Consortium' }),
    ).toBeInTheDocument();
  });

  it('Dismiss resolves the finding without assigning', async () => {
    m.getLintFindings.mockResolvedValue([collision()]);
    render(MaintenanceView, {});
    await fireEvent.click(await screen.findByRole('tab', { name: /Findings/ }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
    expect(m.resolveLintFinding).toHaveBeenCalledWith('lint_finding:c1');
    expect(m.resolveAliasCollision).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm -C apps/desktop exec vitest run src/views/MaintenanceView.test.ts -t "naming-conflict"`
Expected: FAIL — card still renders `faction:a`; no "Keep on …"/"Dismiss" buttons.

- [ ] **Step 3: Import the command and add the handler**

In `MaintenanceView.svelte`, add `resolveAliasCollision` to the import block (lines 4-18):

```ts
    resolveAliasCollision,
```

Add this handler near `openMerge` (~line 190):

```ts
  async function resolveCollision(f: LintFinding, keepId: string, dropId: string) {
    busy = f.id;
    try {
      await resolveAliasCollision(f.id, keepId, dropId);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  /** Prefer the enriched name; fall back to the record id for deleted parties. */
  function partyName(f: LintFinding, side: 'a' | 'b'): string {
    const name = f.payload[`${side}_name`];
    if (typeof name === 'string' && name) return name;
    return entityRef(f.payload[side])?.id ?? String(f.payload[side]);
  }
```

- [ ] **Step 4: Rewrite the `alias_collision` branch**

Replace the whole `{:else if kind === 'alias_collision'}` block (lines 460-490) with:

```svelte
                  {:else if kind === 'alias_collision'}
                    {@const aName = partyName(f, 'a')}
                    {@const bName = partyName(f, 'b')}
                    {@const aIsName = f.payload.a_is_name === true}
                    {@const bIsName = f.payload.b_is_name === true}
                    <p class="finding-detail">
                      <strong>{String(f.payload.alias)}</strong> is claimed by two entities:
                    </p>
                    <div class="conflict-parties">
                      <div class="party">
                        <span class="party-name">{aName}</span>
                        <span class="party-kind">{entityRef(f.payload.a)?.kind ?? ''}</span>
                        <span class="party-tag">{aIsName ? 'as name' : 'as alias'}</span>
                      </div>
                      <div class="party">
                        <span class="party-name">{bName}</span>
                        <span class="party-kind">{entityRef(f.payload.b)?.kind ?? ''}</span>
                        <span class="party-tag">{bIsName ? 'as name' : 'as alias'}</span>
                      </div>
                    </div>
                    <div class="finding-actions">
                      {#if !bIsName}
                        <button
                          type="button"
                          disabled={busy === f.id}
                          onclick={() =>
                            resolveCollision(f, String(f.payload.a), String(f.payload.b))}
                        >
                          Keep on {aName}
                        </button>
                      {/if}
                      {#if !aIsName}
                        <button
                          type="button"
                          disabled={busy === f.id}
                          onclick={() =>
                            resolveCollision(f, String(f.payload.b), String(f.payload.a))}
                        >
                          Keep on {bName}
                        </button>
                      {/if}
                      <button type="button" disabled={busy === f.id} onclick={() => openMerge(f)}>
                        Merge…
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.a)}
                      >
                        Open {aName}
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.b)}
                      >
                        Open {bName}
                      </button>
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => resolveFinding(f.id)}
                      >
                        Dismiss
                      </button>
                    </div>
```

- [ ] **Step 5: Show names on the `duplicate_entity` card**

Replace the `duplicate_entity` detail paragraph (lines 427-432) with name-based display:

```svelte
                    <p class="finding-detail">
                      Possible duplicate:
                      <strong>{partyName(f, 'a')}</strong>
                      and
                      <strong>{partyName(f, 'b')}</strong>
                    </p>
```

- [ ] **Step 6: Add card styles**

In the `<style>` block (after `.finding-detail`, ~line 694), add:

```css
  .conflict-parties {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 10px;
  }
  .party {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .party-name {
    color: var(--fg-1);
    font-weight: 600;
  }
  .party-kind {
    font-size: 0.7rem;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .party-tag {
    font-size: 0.7rem;
    color: var(--fg-3);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 1px 8px;
  }
```

- [ ] **Step 7: Run the naming-conflict tests + full view suite**

Run: `pnpm -C apps/desktop exec vitest run src/views/MaintenanceView.test.ts`
Expected: PASS (new suite + existing tests still green)

- [ ] **Step 8: Lint + typecheck**

Run: `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add apps/desktop/src/views/MaintenanceView.svelte apps/desktop/src/views/MaintenanceView.test.ts
git commit -m "feat(maintenance): resolvable naming-conflict card"
```

---

## Task 7: Acceptance (BDD) — resolve a naming conflict

**Files:**
- Modify: `apps/desktop/tests/e2e/features/maintenance-inbox.feature`
- Modify: `apps/desktop/tests/e2e/backend/steps/maintenance.steps.ts`

- [ ] **Step 1: Add the scenario**

Append to `maintenance-inbox.feature`:

```gherkin
  Scenario: A naming conflict is resolved by assigning the term to one entity
    Given the inbox has a naming conflict for "consortium" between "Merchant Consortium" and "Trade Consortium"
    When the GM opens the findings tab
    Then the finding "Naming conflict" is listed with "Merchant Consortium"
    When the GM keeps the term on "Merchant Consortium"
    Then the resolve-collision command keeps "faction:a" and drops "faction:b"
```

- [ ] **Step 2: Add the step definitions**

Append to `maintenance.steps.ts` (uses the same `installIpcMock` / `getIpcCalls` helpers already imported at the top of the file):

```ts
Given(
  'the inbox has a naming conflict for {string} between {string} and {string}',
  async ({ page }, term: string, nameA: string, nameB: string) => {
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint3',
          kind: 'alias_collision',
          payload: {
            alias: term,
            a: 'faction:a',
            b: 'faction:b',
            a_name: nameA,
            b_name: nameB,
            a_is_name: false,
            b_is_name: false,
          },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      resolve_lint_finding: null,
      resolve_alias_collision: null,
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

When('the GM keeps the term on {string}', async ({ page }, name: string) => {
  await page.getByRole('button', { name: `Keep on ${name}` }).click();
});

Then(
  'the resolve-collision command keeps {string} and drops {string}',
  async ({ page }, keepId: string, dropId: string) => {
    const calls = await getIpcCalls(page);
    const call = calls.find(
      (c) =>
        c.cmd === 'resolve_alias_collision' &&
        c.args?.keepId === keepId &&
        c.args?.dropId === dropId,
    );
    expect(call).toBeDefined();
  },
);
```

> Note: confirm `installIpcMock` and `getIpcCalls` are the helper names imported at the top of `maintenance.steps.ts` (they are used by the existing broken-wikilink/resolve steps). If `expect` is not already imported in this file, add it from `@playwright/test`.

- [ ] **Step 3: Run the backend E2E suite**

Run: `pnpm -C apps/desktop exec playwright test tests/e2e/backend/ -g "naming conflict"`
Expected: PASS (new scenario). Then run the full maintenance feature to confirm no regressions:
`pnpm -C apps/desktop exec playwright test tests/e2e/backend/ -g "Codex write-back review"`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/tests/e2e/features/maintenance-inbox.feature \
        apps/desktop/tests/e2e/backend/steps/maintenance.steps.ts
git commit -m "test(e2e): resolve naming conflict acceptance scenario"
```

---

## Task 8: Full CI gate

Run every check locally before opening a PR (project rule: run the full `ci.yml` gate, including `cargo deny check`).

- [ ] **Step 1: Rust format + lint + test**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```
Expected: all PASS, zero clippy warnings.

- [ ] **Step 2: Security/licence gate**

```bash
cargo audit && cargo deny check
```
Expected: PASS (no new advisories/licences — this change adds no dependencies).

- [ ] **Step 3: Frontend gate**

```bash
pnpm -C apps/desktop typecheck
pnpm -C apps/desktop lint
pnpm -C apps/desktop test:run
pnpm -C apps/desktop exec playwright test tests/e2e/backend/
```
Expected: all PASS.

- [ ] **Step 4: Verify in the real app**

Launch the app (`pnpm -C apps/desktop tauri dev`), open a campaign with a naming conflict, confirm: the findings list scrolls; the card shows real names; "Keep on …" removes the alias and the finding disappears; "Dismiss" clears it. (Per the "run the real app on real data" rule — a green suite is not proof the feature works.)

- [ ] **Step 5: Open the PR**

Push the branch and open a PR describing what/why/how-tested, linking the spec and this plan.

---

## Self-Review Notes

- **Spec coverage:** Part 1 (scroll) → Task 5. Part 2 (read-time name/flag enrichment, soft-delete fallback) → Task 1. Part 3 (card redesign, all four actions, "both are names" collapse) → Task 6. New `resolve_alias_collision` command → Tasks 2–4. Testing section → Tasks 1, 2, 5, 6, 7. `duplicate_entity` name display → Task 6 Step 5.
- **Kind display:** the spec listed `a_kind`/`b_kind` as injected fields; the plan derives kind from the record id via the existing `entityRef()` helper instead (no backend field needed), keeping enrichment lean. `a_summary`/`b_summary` are injected but only wired into the DOM if a later polish task wants them — not required by any acceptance criterion.
- **Type consistency:** `resolve_alias_collision(finding_id, keep_id, drop_id)` is used identically in service (Task 2), command (Task 3), and JS wrapper `resolveAliasCollision(findingId, keepId, dropId)` (Task 4); IPC arg casing (`keepId`→`keep_id`) matches the existing `undoAutoAlias` convention.
- **No placeholders:** every code step shows complete code.
