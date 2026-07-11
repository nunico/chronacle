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
