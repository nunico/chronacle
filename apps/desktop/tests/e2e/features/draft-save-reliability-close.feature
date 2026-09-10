Feature: Protect retained drafts during normal window closing
  A GM can move among Oracle, notes, and reference material without losing work,
  and can tell whether each edit is pending, saved, or needs attention.

  Background:
    Given draft reliability test data is available
    And I have opened Chronacle in campaign "Campaign A"

  @native-close-contract
  Scenario: Cancel or confirm normal window closing with retained drafts
    Given an unsent Oracle question has focus
    When a normal window close is requested
    Then closing is paused by an unsaved-work dialog
    And Cancel has focus
    When I press Escape
    Then the dialog closes
    And focus returns to the Oracle composer
    When I request window closing again
    And I activate "Discard and close" with the keyboard
    Then the retained draft is discarded and window closing proceeds
    And no draft is submitted or saved during closing

  @native-close-contract
  Scenario: Wait for an active save before discarding and closing
    Given a save of an earlier session draft revision is in progress
    And I have made a newer unsaved edit to the session
    When a normal window close is requested
    Then closing is paused by an unsaved-work dialog
    And Discard and close is unavailable
    And I am told to wait for saving to finish before closing
    When the earlier session save completes
    Then closing remains paused
    And my newer session edit remains unsaved
    And Discard and close becomes available
    When I activate "Discard and close" with the keyboard
    Then window closing proceeds
    And closing did not start another save

  Scenario: Opening a close decision does not start another automatic save
    Given a save of an earlier session draft revision is in progress
    And I make a newer focused edit before the close decision
    When an application close decision opens in the browser contract
    Then closing is paused by the browser unsaved-work dialog
    And only the earlier session save has started
    And the newer session edit remains unsaved
