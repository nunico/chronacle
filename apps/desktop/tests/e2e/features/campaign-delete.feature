Feature: Campaign deletion modes
  Deleting a campaign must never silently destroy its owned collection.
  The GM chooses: cascade-delete the notes, or keep them as a regular
  collection (ADR-010).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app

  Scenario: The GM must choose what happens to the campaign's notes
    When the GM opens the campaign manager
    And the GM clicks delete on the campaign "Test Campaign"
    Then a dialog offers "Delete campaign and its notes" and "Keep notes as a regular collection"

  Scenario: Cancelling the dialog deletes nothing
    When the GM opens the campaign manager
    And the GM clicks delete on the campaign "Test Campaign"
    And the GM cancels the dialog
    Then no delete command was sent to the backend
