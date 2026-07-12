# Codex Tranche 4 — Kickoff Prompt (D-series: Markdown Vault Sync)

> Paste the block below into a fresh session to start the next Codex tranche.
> Context: the A/B/C series have all landed on `main` (PRs #4–#17 merged).
> The D-series (vault sync, ADR-008) is the remaining designated tranche and
> is **sketch-only** in the spec — so it starts with brainstorming + planning,
> not straight execution.

---

```
Work on the next tranche of the "Compiled World Model — The Codex" feature:
the D-series — Markdown Vault Sync (ADR-008). The A/B/C series have all
landed on main (PRs #4–#17 merged); this is the remaining designated tranche.

IMPORTANT — this tranche is only SKETCHED in the spec, not pre-planned like
A/B/C. So do NOT jump to implementation. Work in this order:
  1. superpowers:brainstorming — resolve the open design questions before any
     code (see below).
  2. superpowers:writing-plans — produce docs/superpowers/plans/<date>-codex-
     tranche-4-plan.md with bite-sized, TDD, stacked-PR tasks and Gherkin
     acceptance criteria (ADR-011), the same shape as the tranche-3 plan.
  3. superpowers:subagent-driven-development — execute it (fresh implementer
     per task, task review after each, final whole-branch review), the same
     way tranche 3 was run.

Authoritative sources to read first:
  - Design spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-
    design.md — the "D-series (sketch only)" note (~line 417) and "Open
    questions".
  - ADR-008 in docs/architecture.md (~line 509): VaultSyncService, the vault
    directory layout, bidirectional sync table (outbound + inbound), soft-
    delete via vault_deleted, the notify::RecommendedWatcher, vault_sync_path
    / vault_include_gm_only settings.
  - The tranche-3 plan (docs/superpowers/plans/2026-07-05-codex-tranche-3-
    plan.md) as a structural template.

Scope this tranche covers (confirm/narrow during brainstorming):
  - Outbound sync: export codex articles + rule_entries (+ entity notes) as
    .md into a user-configured vault dir; full-reconcile pass + incremental
    on change.
  - Inbound sync: notify watcher; file edit → update record; file delete →
    soft-delete (vault_deleted=TRUE) + "restore or confirm" UI; file move →
    remap entity_type.
  - The vault_sync_path Tauri command + settings UI.

Design questions to settle in brainstorming BEFORE planning:
  - Conflict resolution when both sides changed (last-writer-wins by
    updated_at vs. explicit merge?).
  - Whether inbound edits to compiler-owned article bodies are allowed or
    only GM notes (mirror the rule_entry "body is compiler-owned, notes are
    GM-owned" split).
  - Player-safe export is gated on Phase-3 AI-detected GM-secret flags, which
    DON'T exist yet — so this tranche exports GM-visible only, gated by
    vault_include_gm_only. Confirm we're not reintroducing a manual is_gm_only
    flag (that was built and reverted; GM-secret is Phase 3, AI-detected).

Hard constraints carried from prior tranches (all still binding):
  - Traits for all external deps (Arc<dyn ...>, Mock* in tests); Tauri IPC
    only; SurrealQL only; migrations are DEFINE-only/idempotent (re-run every
    boot); Svelte 5 runes only; approved-crates-only (a new crate like
    `notify` needs an ADR + architecture "Crate & Tool Summary" entry —
    ADR-008 already names notify, but confirm it's approved before adding).
  - Embedding-model identity preserved on any re-index; FLEXIBLE object
    fields: never bind serde_json::Value on writes.
  - Filesystem access only through the BlobStore trait / a new vault trait —
    never touch std::fs directly in service logic.

Workflow constraints (learned the hard way in tranche 3):
  - Stacked feature branches; push new/stacked branches with an EXPLICIT
    refspec ("${b}:refs/heads/${b}") because push.default=upstream mis-pushes
    onto the parent PR. Reconcile the stack after each PR merges (git rebase
    --onto main <old-parent> <branch>, force-push --force-with-lease, retarget
    next PR base -> main).
  - Before opening/updating ANY PR, run the FULL ci.yml gate locally and
    confirm green — including `cargo deny check` (advisories/bans/licenses/
    sources), which is easy to forget and is time-dependent. Full gate:
    cargo fmt --all --check · cargo clippy --workspace --all-targets
    --all-features -D warnings · cargo test --workspace · cargo deny check ·
    pnpm -C apps/desktop {typecheck,lint,test:run,run e2e:backend}.
  - Every user-visible behavior ships .feature acceptance scenarios in the
    same PR (ADR-011).

Start by reading the spec's D-series sketch + ADR-008, then open brainstorming
to resolve the design questions above.
```

---

## Deferred items not in this tranche's core scope

These were parked during the C series; decide during brainstorming whether any
fold into D or wait for their own tranche:

- **LLM-driven `contradiction` lint** — explicitly deferred in the spec
  (expensive, noisy); future work, not part of vault sync.
- **Entity merge for `duplicate_entity`** — C2b links to both entities and
  defers merge; a real merge operation is still open.
- **Manual proposal drafting escape hatch** from entity/rule pages — the
  table + service already support it; only the UI affordance is missing, and
  it can ride any later PR.
