Feature: Save session drafts reliably while moving through a campaign
  A GM can move among Oracle, notes, and reference material without losing work,
  and can tell whether each edit is pending, saved, or needs attention.

  Background:
    Given draft reliability test data is available
    And I have opened Chronacle in campaign "Campaign A"

  Scenario: Continue editing during a save
    Given a save of an earlier session draft revision is in progress
    When I make another edit to the session
    And the earlier save completes
    Then my newer session edit remains intact
    And it is not incorrectly marked as saved

  Scenario: Do not infer acknowledgment from coincidentally equal content
    Given a save of session draft revision 1 is in progress
    When I make session draft revision 2
    And revision 1 is acknowledged with canonical content equal to revision 2
    Then revision 2 remains visible as unsaved
    When I save revision 2 and its acknowledgment completes
    Then revision 2 is shown as saved

  Scenario: Recover a session edit after saving fails
    Given I have changed a session title
    When the session save fails
    Then the changed session title remains available
    And the session shows an actionable save failure
    When I retry the session save and it succeeds
    Then the changed session title is shown as saved
    And the session failure indication is cleared

  Scenario: Keep an in-flight session draft separate between campaigns
    Given a save of my changed session title is in progress
    When I switch to campaign B
    Then campaign A's session draft is not shown in campaign B
    When I return to campaign A
    Then the changed session title is preserved
    And its save is still in progress

  Scenario: Reverting a session edit to its saved baseline clears pending state
    Given I have changed a session title without blurring it
    When I restore the original session title
    Then the session no longer indicates unsaved changes
    And no session save has been sent

  Scenario: Preserve a session draft when its target is unavailable
    Given I have changed a session title
    When saving reports that the session is no longer available
    Then the changed session title remains available
    And the session shows the localized unavailable-target recovery
    And the raw session backend detail is not shown
    When I retry the unavailable session save
    Then Retry still targets the same session

  Scenario: Do not let a session list requested before acknowledgment restore old content
    Given a save of my changed session title is in progress
    And the next Campaign A session list completes with the earlier saved title
    When I navigate to Oracle before the save completes
    And I return to Sessions before the held list completes
    And the pending session save completes
    And the held session list completes
    Then the changed session title is preserved
    And it is shown as saved
    When a later session list loads newer canonical content
    Then the newer canonical session title is shown

  Scenario: Ignore a session list from an obsolete same-campaign view
    Given the next Campaign A session list is held with obsolete content
    When I open Sessions and leave before that list completes
    And the backend session gains newer canonical content
    And I return to Sessions
    Then the newer canonical session title is shown
    When the obsolete session list completes
    Then the newer canonical session title is still shown

  Scenario: Wait for a session save before deleting it
    Given I have an unsent question in campaign A
    And a save of an earlier session draft revision is in progress
    Then Delete is unavailable for that session
    And I am told to wait for the session save to finish
    When the earlier save completes
    And I activate Delete and explicitly confirm deletion
    Then the session and only its retained draft are removed

  Scenario: Prevent session writes while confirmed deletion is pending
    Given I have an unsent question in campaign A
    And a session save has failed while its title field is focused
    When I confirm deleting the session and deletion remains in progress
    Then the session fields and its Delete, Retry, and Discard actions are unavailable
    When I attempt to retry while the session deletion is pending
    Then no session save starts behind deletion
    When the pending session deletion completes
    Then the session and only its retained draft are removed

  Scenario: Discard a session draft with the keyboard
    Given I have an unsent question in campaign A
    And I have changed a session title without blurring it
    When I activate the session Discard action with the keyboard
    Then the saved session title is restored
    And focus moves to the stable session header
    And the unsent Oracle question remains intact

  Scenario: Navigate while an automatic save is pending
    Given a save of my changed session title is in progress
    When I navigate to Oracle before the save completes
    And the pending session save completes
    And I return to Sessions
    Then the changed session title is preserved
    And it is shown as saved

  Scenario: Retain a focused session edit when navigating to another view
    Given I have changed a session title without blurring it
    When I navigate to Oracle
    Then no session save has been sent
    When I return to Sessions
    Then the changed session title is preserved
    And it remains shown as unsaved

  Scenario: Coalesce multiple rapid saves without overwriting newer content
    Given session saves are being held open
    When I request rapid saves for three different session titles
    Then the session save attempts do not overlap
    When the pending session saves are acknowledged
    Then the newest session title is preserved
    And only the newest revision is shown as saved

  Scenario: Retry a failed save with the keyboard
    Given a session save has failed while its title field is focused
    When I move to Retry and press Enter
    Then the session save is retried
    And focus returns to the session title

  Scenario: Finish creating a session after switching campaigns
    Given creating a new session for campaign A is in progress
    When I switch to campaign B before session creation completes
    And the delayed session creation succeeds
    Then the created session is not shown in campaign B
    When I return to campaign A sessions
    Then the created session is shown once in campaign A
    And the created session belongs to campaign A

  Scenario: Keep a created session when an older Campaign A list completes last
    Given creating a new session for campaign A is in progress
    When I switch to campaign B before session creation completes
    And I return to campaign A while its older session list is held
    And the delayed session creation succeeds
    And the older Campaign A session list completes without the created session
    Then the created session is shown once in campaign A
    And the created session belongs to campaign A

  Scenario: Do not duplicate a session listed before its create acknowledgment
    Given creating a new session for campaign A is in progress
    When I switch to campaign B before session creation completes
    And the backend commits the session without acknowledging creation
    And I return to campaign A sessions
    Then the created session is shown once in campaign A
    When the delayed session creation acknowledgment arrives
    Then the created session is still shown once in campaign A
    And the created session belongs to campaign A

  Scenario: Recover a failed new-session creation in its original campaign
    Given creating a default new session for campaign A fails
    Then the session view keeps an actionable creation failure
    When I switch to campaign B and return to campaign A sessions
    Then the failed creation is restored only in campaign A
    When I retry the failed creation twice with the keyboard while it is pending
    Then only one retry uses the original Campaign A session details
    When the retried session creation succeeds
    Then the session is created once in campaign A
    And the creation failure is cleared

  Scenario: Recover and discard a session omitted after a failed save
    Given saving a changed session failed before its target was deleted
    And an unrelated session draft is retained
    And that session target is now deleted
    When I retry the failed session and it reports unavailable
    And I navigate away and return to Sessions
    Then the changed session remains available as unavailable
    And its exact title and notes are retained
    When I retry the omitted session with the keyboard
    Then Retry still targets the omitted session
    And the unavailable failure remains actionable
    When I discard the omitted session with the keyboard
    Then only the omitted session recovery row disappears
    And focus moves to the stable session control
    And the unrelated session draft remains intact

  Scenario: Ignore navigation shortcuts while editing, then save before keyboard navigation
    Given I have changed a session title without blurring it
    When I press the Oracle g chord in the focused session field
    Then I remain in the session editor and the typed keys remain unsaved
    And no session save has been sent
    When I move focus to the session notes field
    Then the ordinary session blur is saved
    When I navigate to Oracle with the g chord
    And I return to Sessions with the g chord
    Then the exact keyboard-edited session title is restored
    And it is shown as saved
