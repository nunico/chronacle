Feature: App shell smoke
  The BDD harness (ADR-011) drives the frontend with mocked Tauri IPC.
  This feature proves the toolchain wiring end to end.

  Scenario: GM opens the app and reaches the Oracle
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app
    Then the topbar shows the app title "Oracle"
