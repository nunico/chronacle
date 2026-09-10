Feature: Coordinate campaign deletion with retained drafts

  Background:
    Given draft reliability test data is available
    And I have opened Chronacle in campaign "Campaign A"

  Scenario: Disclose and preserve campaign drafts when deletion fails
    Given campaign A contains an unsaved entity draft and a failed session draft
    And campaign B contains an unrelated Oracle draft
    When I open deletion confirmation for campaign A
    Then the campaign deletion discloses both at-risk drafts
    When campaign A deletion fails
    Then campaign A remains available
    And campaign A's exact entity and session drafts remain available
    And campaign B's Oracle draft remains unchanged

  Scenario: Block campaign deletion during an active write
    Given campaign A has an active session save
    When I open deletion confirmation for campaign A
    Then both campaign deletion choices are unavailable
    And I am told to wait for campaign saves to finish
    When the campaign session save succeeds
    Then both campaign deletion choices become available

  Scenario: Delete only the confirmed campaign and its drafts
    Given campaign A contains an unsaved entity draft and a failed session draft
    And campaign B contains an unrelated Oracle draft
    When I open deletion confirmation for campaign A
    And I confirm deleting campaign A and its notes
    Then campaign A is deleted
    And campaign A's retained drafts are removed
    And campaign B remains available with its Oracle draft unchanged
    And no removed campaign draft blocks a later close
