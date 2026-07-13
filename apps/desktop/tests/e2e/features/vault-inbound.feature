Feature: Inbound vault sync
  The GM edits campaign files in their Markdown vault; changes flow back into
  Chronacle. Divergent edits become .conflict.md sidecars resolved in the vault.

  # ── Backend-only invariants ──────────────────────────────────────────────
  # Every scenario below drives a real GM edit against the vault's Markdown
  # files, which the mocked-IPC backend suite has no filesystem to perform.
  # Each step arranges the mock to reflect what a real `vault_sync_now` would
  # report and read back, so the scenarios still prove the UI's half of the
  # contract (it dispatches the sync, and it renders whatever the backend
  # returns). The deep half — the actual file write, fence revert, sidecar
  # bytes, and DB round-trip — is proven end-to-end in Rust; each scenario is
  # annotated with the covering test.

  # backend: covered by apps/desktop/src-tauri/tests/vault_inbound.rs::gm_edit_round_trips_through_reconcile_into_the_db
  Scenario: A vault edit updates the record
    Given a synced vault with an entity "Seraphina Aldric"
    When the GM edits the notes of "Seraphina Aldric" in the vault
    And a sync runs
    Then the entity "Seraphina Aldric" has the edited notes in Chronacle

  # backend: covered by crates/chronacle-vault/src/reconcile.rs::reconcile_applies_an_inbound_edit_and_reexports_canonical
  Scenario: An edit inside the compiled block is reverted
    Given a synced vault with an entity "Seraphina Aldric"
    When the GM edits inside the compiled block of "Seraphina Aldric"
    And a sync runs
    Then the vault file of "Seraphina Aldric" shows the compiled text again

  # backend: covered by apps/desktop/src-tauri/tests/vault_inbound.rs::conflict_freezes_then_sidecar_deletion_resolves_to_the_file_version
  # backend: covered by apps/desktop/src-tauri/tests/vault_inbound.rs::conflicts_lists_a_frozen_record_end_to_end
  Scenario: Divergent edits produce a conflict sidecar
    Given a synced vault with an entity "Seraphina Aldric"
    When both Chronacle and the vault file of "Seraphina Aldric" are edited differently
    And a sync runs
    Then a conflict sidecar exists for "Seraphina Aldric"
    And the vault sync settings list "Seraphina Aldric" as a conflict

  # backend: covered by apps/desktop/src-tauri/tests/vault_inbound.rs::conflict_freezes_then_sidecar_deletion_resolves_to_the_file_version
  Scenario: Deleting the sidecar resolves the conflict with the vault version
    Given an entity "Seraphina Aldric" frozen in conflict
    When the GM deletes the conflict sidecar
    And a sync runs
    Then the entity "Seraphina Aldric" has the vault version in Chronacle
    And no conflict is listed for "Seraphina Aldric"

  # backend: covered by crates/chronacle-vault/src/reconcile.rs::reconcile_soft_deletes_a_record_whose_file_is_gone
  Scenario: Deleting a vault file soft-deletes the record
    Given a synced vault with an entity "Seraphina Aldric"
    When the GM deletes the vault file of "Seraphina Aldric"
    And a sync runs
    Then "Seraphina Aldric" is no longer visible in Chronacle

  # backend: covered by apps/desktop/src-tauri/tests/vault_inbound.rs::switching_to_a_fresh_dir_after_clearing_bases_exports_cleanly
  Scenario: Switching vault folders deletes nothing
    Given a synced vault with an entity "Seraphina Aldric"
    When the vault path is changed to a new empty folder
    Then "Seraphina Aldric" is still visible in Chronacle
    And the new folder contains a file for "Seraphina Aldric"
