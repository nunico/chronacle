Feature: Codex compilation
  The GM compiles a collection's codex explicitly; staleness badges show
  what is pending (ADR-009: manual compile, never automatic).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app
    And the GM opens the campaign manager

  Scenario: A collection with stale entities shows its badge and compiles
    Then the collection "World Guide" shows the codex badge "12 stale"
    When the GM clicks compile on the collection "World Guide"
    Then the compile command is sent for the collection "World Guide"
