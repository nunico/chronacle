Feature: Markdown vault sync settings

  Scenario: No vault configured
    Given no vault is configured
    When the GM opens Settings
    Then the settings page shows "No vault configured"
    And the "Sync now" button is disabled

  Scenario: Vault configured
    Given a vault is configured at "/Users/gm/Vault"
    When the GM opens Settings
    Then the settings page shows the vault path "/Users/gm/Vault"
    And the "Sync now" button is enabled

  Scenario: Syncing now reports the reconcile counts
    Given a vault is configured at "/Users/gm/Vault"
    And a sync will report 3 exported and 2 failed
    When the GM opens Settings
    And the GM clicks "Sync now"
    Then the settings page shows "3 exported"
    And the settings page shows "2 failed"

  Scenario: Disconnecting clears the vault path
    Given a vault is configured at "/Users/gm/Vault"
    When the GM opens Settings
    And the GM clicks "Disconnect"
    Then the set vault path command was sent with null

  # ── D4b: live vault export from every record producer ──────────────────────
  # These three scenarios exercise the frontend actions that trigger the backend
  # outbound enqueue → vault write. The mocked-IPC backend suite has no real
  # filesystem, so each asserts the producer IPC the UI dispatches — the exact
  # trigger of the backend write. The file-level effects (body updates,
  # one-write-per-file, rename re-keys the file) are verified in Rust by
  # vault_outbound_test.rs (enqueue-per-producer) and chronacle-vault's
  # index-aware drain, plus the tauri-driver UI suite.

  Scenario: Editing an entity's notes dispatches the vault-producing update
    Given a vault is configured at "/Users/gm/Vault" and an entity "Seraphina Aldric"
    When the GM edits that entity's notes to "She guards the archive."
    Then an update entity command was sent with notes "She guards the archive."

  Scenario: Renaming an entity dispatches the vault-producing update
    Given a vault is configured at "/Users/gm/Vault" and an entity "Seraphina Aldric"
    When the GM renames that entity to "Seraphina the Archivist"
    Then an update entity command was sent with name "Seraphina the Archivist"

  Scenario: Recompiling a collection dispatches exactly one compile
    Given a vault is configured at "/Users/gm/Vault" and a compiled collection "World Guide"
    When the GM recompiles the collection
    Then exactly one compile collection command was sent
