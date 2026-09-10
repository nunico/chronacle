Feature: Save rule-note drafts reliably while moving through a campaign
  A GM can move among Oracle, notes, and reference material without losing work,
  and can tell whether each edit is pending, saved, or needs attention.

  Background:
    Given draft reliability test data is available
    And I have opened Chronacle in campaign "Campaign A"

  Scenario: Recover a rule note after saving fails
    Given I have changed the table notes for rule "Initiative"
    When the rule-note save fails
    Then the changed rule note remains available
    And the rule note shows an actionable save failure
    When I retry the rule-note save and it succeeds
    Then the changed rule note is shown as saved
    And the rule-note failure indication is cleared

  Scenario: Retain a rule note while navigating and keep campaigns separate
    Given a save of my changed rule note is in progress
    When I navigate to Oracle before the rule-note save completes
    And I return to the Initiative rule
    Then the changed rule note is preserved and shown as saving
    When I switch to campaign B
    And I return to the Initiative rule
    Then campaign A's changed rule note is not shown in campaign B
    When I return to campaign A
    And I return to the Initiative rule
    Then the changed rule note is preserved and shown as saving

  Scenario: Retain a focused rule note without saving it during navigation
    Given I have entered "Unsaved note" in the focused Initiative table notes
    When I navigate to Oracle while the rule note is still focused
    Then no rule-note save has been sent
    When I return to the Initiative rule
    Then the exact rule note "Unsaved note" is restored
    And the rule note is shown as unsaved

  Scenario: Reverting a rule note to its saved baseline clears pending state
    Given I have entered "Unsaved note" in the focused Initiative table notes
    And the rule note is shown as unsaved
    When I restore the saved Initiative table note
    Then the rule note no longer indicates unsaved changes
    And no rule-note save has been sent

  Scenario: Keep retained rule-note saves separate between collections
    Given a World Guide Initiative rule-note save is in progress
    When I navigate to Oracle before the collection rule-note save completes
    And I open Initiative in the Adventurer Guide
    Then the World Guide rule-note draft is not shown in the Adventurer Guide
    And the Adventurer Guide saved rule note is shown
    When I return to Initiative in the World Guide
    Then the World Guide rule-note draft is preserved and shown as saving

  Scenario: Keep a rule draft separate from the explicit no-campaign context
    Given no campaign is available
    And I have entered "No-campaign rule note" in the focused Initiative table notes
    When I create campaign "Campaign A"
    And I open Initiative in the World Guide
    Then the no-campaign rule-note draft is not shown in campaign A
    And the saved Initiative table note is shown
    When I delete campaign "Campaign A" and return to Initiative
    Then the exact rule note "No-campaign rule note" is restored
    And the rule note is shown as unsaved
    And no rule-note save has been sent

  Scenario: Continue editing a rule note during a save
    Given a save of an earlier rule-note revision is in progress
    When I make a newer edit to the rule note
    And the earlier rule-note save completes
    Then my newer rule-note edit remains intact
    And the rule note is not incorrectly marked as saved

  Scenario: Reject an acknowledgment for another rule
    Given saving my changed Initiative note is in progress
    When that save is acknowledged as the Adventurer Guide Initiative rule
    Then my changed Initiative note remains available and needs attention
    And neither rule is overwritten by the wrong acknowledgment
    When I retry the Initiative note and its acknowledgment succeeds
    Then the changed Initiative note is saved to Initiative only

  Scenario: Coalesce rapid rule-note saves without overlapping writes
    Given rule-note saves are being held open
    When I request rapid saves for three different rule notes
    Then the rule-note save attempts do not overlap
    When the pending rule-note saves are acknowledged
    Then the newest rule note is preserved
    And only the newest rule-note revision is shown as saved

  Scenario: Keep an unavailable rule-note failure on its row and target
    Given I have changed the table notes for rule "Initiative"
    When saving reports that rule "Initiative" is no longer available
    Then the changed rule note remains available
    And the rule note shows an actionable unavailable-target failure
    When I retry the unavailable rule-note save with the keyboard
    Then Retry still targets rule "Initiative"
    And focus remains on the rule-note Retry action

  Scenario: Recover an unavailable rule note omitted by a later list
    Given I have an unsent question in campaign A
    And I have changed the table notes for rule "Initiative"
    When saving reports that rule "Initiative" is no longer available
    And I navigate away and reload the rule list
    Then the omitted Initiative draft remains available as recovery-only
    And the unavailable rule-note recovery is localized and actionable
    And the raw rule backend detail is not shown
    When I retry the omitted rule-note save with the keyboard
    Then Retry uses the original Initiative target and content
    When I discard the omitted rule-note draft with the keyboard
    Then only the omitted Initiative draft is removed
    And focus moves to the stable Rules tab
    And the unsent Oracle question remains intact

  Scenario: Preserve queued rule saves from each campaign sharing one target
    Given a Campaign A Initiative rule-note save is in progress
    When Campaign B requests its Initiative rule-note save
    And Campaign A requests a newer Initiative rule-note save
    Then Initiative rule-note writes do not overlap
    When all three Initiative rule-note writes are acknowledged
    Then each campaign's requested rule-note write was preserved in order
    And the newest Campaign A rule note is persisted as saved

  Scenario: Ignore navigation shortcuts while editing, then save before keyboard rule navigation
    Given I have entered "Unsaved note" in the focused Initiative table notes
    When I press the Oracle g chord in the focused rule-note field
    Then I remain in the Initiative rule and the typed keys remain unsaved
    And no rule-note save has been sent
    When I move focus to the Initiative redo control
    Then the ordinary rule-note blur is saved
    When I navigate to Oracle with the g chord
    And I return to the Initiative rule with the keyboard
    Then the exact keyboard-edited rule note is restored
    And the rule note is shown as saved
