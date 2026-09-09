Feature: Preserve work while moving through a campaign
  A GM can move among Oracle, notes, and reference material without losing work,
  and can tell whether each edit is pending, saved, or needs attention.

  Background:
    Given draft reliability test data is available
    And I have opened Chronacle in campaign "Campaign A"

  Scenario: Return to an unsent question
    Given I have entered an Oracle question without sending it
    When I open the campaign notebook
    And I return to Oracle
    Then my question is still in the composer
    And it has not been submitted

  Scenario: Keep question drafts separate between campaigns
    Given I have an unsent question in campaign A
    When I switch to campaign B
    Then campaign A's question is not shown in campaign B
    When I enter a different unsent question in campaign B
    And I return to campaign A
    Then its unsent question is restored
    And neither question has been submitted

  Scenario: Retain an unsent question without a campaign
    Given no campaign is available
    And I enter an Oracle question without sending it
    When I open Settings
    And I return to Oracle
    Then my no-campaign question is still in the composer
    And it has not been submitted

  Scenario: Return to an edited entity
    Given I have changed the notes for entity "Mira" without saving
    When I navigate to another view
    And I reopen entity "Mira"
    Then my entity edits are preserved
    And the interface indicates that they are not yet saved

  Scenario: Do not restore an old list row after a save acknowledgment
    Given saving my changed entity "Mira" is held in progress
    When I navigate away and return to NPCs
    And the entity list completes with Mira's earlier saved content
    And the held save completes with its canonical content
    And I open entity "Mira Moonshadow" from that already-rendered list
    Then the acknowledged canonical name and notes are shown in the row, preview, and editor
    And the acknowledged entity is shown as saved
    When a later entity list completes with newer canonical content
    And I open entity "Mira Moonshadow"
    Then the newer canonical content is shown

  Scenario: Ignore an old entity list after its view has closed
    Given the next NPC list load is held with Mira's earlier saved content
    When I open NPCs and leave before that list completes
    And I return to NPCs and save canonical changes to Mira
    And the old NPC list completes after that save acknowledgment
    Then the acknowledged canonical name and notes are shown in the row, preview, and editor
    And the acknowledged entity is shown as saved

  Scenario: Preserve a new entity draft
    Given I have started creating an NPC
    And I have entered its name and notes
    When I navigate away and return to NPCs
    Then my new entity draft is restored
    And navigation has not created a saved entity

  Scenario: Continue editing while an entity is being created
    Given creation of revision 1 for a new NPC is in progress
    When I enter revision 2 before creation completes
    Then the Create action is unavailable
    When creation completes and assigns the NPC an ID
    Then revision 2 remains visible as unsaved
    And the explicit action is now Save
    When I save revision 2
    Then revision 2 updates the NPC with the assigned ID
    And only one NPC has been created

  Scenario: Wait for active creation before replacing a new entity draft
    Given creation of revision 1 for a new NPC is in progress
    When I open entity "Mira"
    And I request creation of the missing NPC link "Aldric"
    Then Discard and create is unavailable while the original creation is saving
    And Discard and create is explicitly described by the wait message
    And Keep editing has focus
    And only one NPC has been created
    When creation completes and assigns the NPC an ID
    Then Discard and create becomes available after creation settles
    When I choose to discard the draft and create
    Then the completed original NPC remains saved without a duplicate creation
    And a new NPC draft is open with the name "Aldric"

  Scenario: Finish creating after navigation remounts the entity editor
    Given creation of revision 1 for a new NPC is in progress
    When I navigate to Oracle
    And I return to NPCs before creation completes
    Then the new NPC draft is restored with Create unavailable
    When creation completes and assigns the NPC an ID
    And the entity list loaded before completion does not contain that ID
    Then the acknowledged NPC remains visible and selected
    And the interface indicates that it is saved
    And the explicit action is now Save
    And only one NPC has been created

  Scenario: Finish creating when the saved record appears before acknowledgment
    Given creation of revision 1 for a new NPC is in progress
    When I enter revision 2 before creation completes
    And the backend commits the new NPC but delays its acknowledgment
    And I navigate to Oracle
    And I return to NPCs before creation is acknowledged
    Then the committed NPC appears in the entity list
    When the delayed creation acknowledgment arrives
    Then revision 2 remains visible as unsaved
    And the explicit action is now Save
    When I save revision 2
    Then revision 2 updates the NPC with the assigned ID
    And only one NPC has been created

  Scenario: Recover when the listed destination has unsaved work
    Given creation of revision 1 for a new NPC is in progress
    And the backend commits the new NPC but delays its acknowledgment
    And I navigate away and return to NPCs
    And I make unsaved changes to the assigned NPC from the list
    When the delayed creation acknowledgment arrives
    Then the listed NPC changes remain unsaved
    And the original new NPC draft remains available
    And I see that the NPC was created but needs attention with Retry
    And only one NPC has been created
    When I explicitly discard the listed NPC changes
    And I retry finishing the created NPC
    Then the original new NPC draft is promoted to the assigned NPC
    And no additional NPC is created
    When I edit and save the promoted NPC
    Then the later edit updates the NPC with the assigned ID
    And only one NPC has been created

  Scenario: Move focus safely after promotion Retry
    Given a created NPC needs attention because its listed destination had unsaved work
    And I have resolved the listed destination changes
    And the promotion Retry action has focus
    When I activate promotion Retry with the keyboard
    Then the original new NPC draft is promoted to the assigned NPC
    And focus moves to the promoted NPC editor

  Scenario: Keep focus when promotion Retry remains blocked
    Given a created NPC needs attention because its listed destination still has unsaved work
    And the promotion Retry action has focus
    When I activate promotion Retry with the keyboard
    Then the promotion failure remains actionable
    And focus remains on the promotion Retry action

  Scenario: Preserve newer saved authority when Create acknowledges last
    Given creation of revision 1 for a new NPC is in progress
    When I enter revision 2 before creation completes
    And the backend commits the new NPC but delays its acknowledgment
    And I navigate away and return to NPCs
    And I update the committed NPC from the entity list
    And that destination Update is acknowledged as saved
    When the delayed creation acknowledgment arrives
    Then the destination Update remains visible as saved
    And the destination Update remains persisted for the assigned NPC
    And the original revision 2 draft remains available
    And I see that the NPC was created but needs attention
    And only one NPC has been created
    When I activate Keep saved record with the keyboard
    Then the original revision 2 draft is discarded without another backend write
    And the destination Update remains visible as saved
    And focus moves to the saved NPC's Save action

  Scenario: Keep converged destination presentation when Create acknowledges last
    Given creation of revision 1 for a new NPC is in progress
    When the backend commits the new NPC but delays its acknowledgment
    And I switch to campaign B and return to campaign A on NPCs
    Then the committed NPC appears in the entity list
    When I rename and save the assigned NPC destination
    And the delayed creation acknowledgment arrives
    Then the converged NPC row keeps the destination name and selection
    And the converged NPC editor and preview keep the destination content
    And the converged NPC is shown as saved
    And the destination content remains persisted for the assigned NPC
    And convergence leaves one created NPC without another backend write
    When I switch from the converged NPC to Mira and back
    Then the assigned NPC editor remains usable with the destination content

  Scenario: Keep my draft after a newer destination is acknowledged
    Given creation of revision 1 for a new NPC is in progress
    When I enter revision 2 before creation completes
    And revision 2 also renames the NPC
    And the backend commits the new NPC but delays its acknowledgment
    And I navigate away and return to NPCs
    And I update the committed NPC from the entity list
    And that destination Update is acknowledged as saved
    When the delayed creation acknowledgment arrives
    Then the renamed revision 2 draft remains available
    And I see that the NPC was created but needs attention
    When I activate Keep my draft with the keyboard
    Then the kept revision 2 name, notes, and preview remain visible as unsaved
    And the saved destination is unchanged without another backend write
    And focus moves to the kept NPC's Save action
    When I save the kept revision 2
    Then the kept revision 2 updates the NPC with the assigned ID exactly once
    And only one NPC has been created

  Scenario: Keep a hidden new draft when creating from a link
    Given I have a dirty new NPC draft
    And I am viewing the existing NPC "Mira"
    When I request creation of the missing NPC link "Aldric"
    Then I am asked whether to keep or discard my new NPC draft
    When I choose to keep editing
    Then my original new NPC draft is reopened unchanged
    And no NPC named "Aldric" has been created

  Scenario: Cancel replacing a hidden new draft with the keyboard
    Given I have a dirty new NPC draft
    And I am viewing the existing NPC "Mira"
    When I request creation of the missing NPC link "Aldric"
    Then Keep editing has focus
    When I press Escape
    Then the replacement confirmation closes
    And focus returns to the Create control that invoked it
    And my original new NPC draft remains unchanged

  Scenario: Replace a hidden new draft when creating from a link
    Given I have a dirty new NPC draft
    And I am viewing the existing NPC "Mira"
    When I request creation of the missing NPC link "Aldric"
    And I choose to discard the draft and create
    Then only my original new NPC draft is discarded
    And a new NPC draft is open with the name "Aldric"
    And no NPC named "Aldric" has been created yet

  Scenario: Keep entity drafts separate while switching records
    Given I have changed the notes for entity "Mira" without saving
    When I open entity "Torvin"
    Then Mira's draft is not shown for Torvin
    When I return to entity "Mira"
    Then my entity edits are preserved
    And Torvin's saved content is unchanged

  Scenario: Reset frontend validation when switching entity records
    Given I have cleared the required name for entity "Mira"
    And I have tried to save entity "Mira"
    Then Mira's name field indicates that it is required
    When I open entity "Torvin"
    Then Torvin's saved name is shown
    And Mira's required-name message is not shown for Torvin
    When I return to entity "Mira"
    Then Mira's blank-name draft remains available as unsaved
    And the frontend required-name message is cleared until I try to save again

  Scenario: Show retained entity state while its form is closed
    Given I have changed the notes for entity "Mira" without saving
    When I open entity "Torvin"
    Then Mira's row indicates unsaved changes
    When I reopen entity "Mira" and start saving
    And I open entity "Torvin" before the save completes
    Then Mira's row indicates saving
    When the save fails
    Then Mira's row indicates that saving failed
    And reopening Mira restores the failed draft and Retry action

  Scenario: Reverting an entity to its saved baseline clears pending state
    Given I have changed the notes for entity "Mira" without saving
    When I restore Mira's notes to the saved value
    Then the interface no longer indicates unsaved changes
    And no save has been sent for Mira

  Scenario: Recover from a failed save
    Given I have unsaved changes to entity "Mira"
    When saving the entity fails
    Then my changes remain available
    And I see an actionable save failure
    When I retry and saving succeeds
    Then the changes are saved to entity "Mira"
    And the failure indication is cleared

  Scenario: Move focus safely when inline Retry disappears
    Given saving my changes to entity "Mira" has failed
    And the inline Retry action has focus
    When I retry and saving succeeds
    Then the failure indication is cleared
    And focus moves to Mira's stable editor control

  Scenario: Keep focus on inline Retry when recovery still fails
    Given saving my changes to entity "Mira" has failed
    And the inline Retry action has focus
    When I retry and saving fails again
    Then the actionable failure remains
    And focus remains on Mira's Retry action

  Scenario: Treat backend validation as a recoverable save failure
    Given I have frontend-valid unsaved changes to entity "Mira"
    When the backend rejects the entity save as invalid
    Then my changes remain available
    And I see an actionable save failure
    When I retry and saving succeeds
    Then the same changes are saved to entity "Mira"
    And the failure indication is cleared

  Scenario: Keep a delayed backend validation failure with its entity
    Given saving frontend-valid changes to entity "Mira" is in progress
    When I open entity "Torvin" before the save completes
    And the backend rejects Mira's save as invalid
    Then Torvin's saved content remains visible without Mira's error
    When I reopen entity "Mira"
    Then Mira's changed content remains available
    And Mira shows the save failure and Retry action

  Scenario: Keep required-field validation local
    Given I have cleared the required name for entity "Mira"
    When I try to save the entity
    Then the name field indicates that it is required
    And Mira remains marked with unsaved changes
    And no entity save has been sent

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

  Scenario: Recover a rule note after saving fails
    Given I have changed the table notes for rule "Initiative"
    When the rule-note save fails
    Then the changed rule note remains available
    And the rule note shows an actionable save failure
    When I retry the rule-note save and it succeeds
    Then the changed rule note is shown as saved
    And the rule-note failure indication is cleared

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

  Scenario: Preserve a draft when its target is unavailable
    Given I have unsaved changes to entity "Mira"
    When saving reports that entity "Mira" is no longer available
    Then my changes remain available
    And I see that the target is unavailable
    And no other entity is changed

  Scenario: Reopen an unavailable entity draft after a list reload
    Given saving my draft for entity "Mira" reports that the target is unavailable
    When I navigate away and the entity list reloads without "Mira"
    And I return to NPCs
    Then an unavailable row for "Mira" indicates that saving failed
    When I open the unavailable row
    Then my changes remain available with Retry
    And Retry still targets entity "Mira"

  Scenario: Explicitly discard changes
    Given I have an unsent question in campaign A
    And I have unsaved changes to entity "Mira"
    When I explicitly discard Mira's changes
    Then Mira's saved version is restored
    And the unsent Oracle question remains intact

  Scenario: Wait for an active entity save before discarding
    Given a save of an earlier entity "Mira" draft revision is in progress
    When I make a newer edit to entity "Mira"
    Then Discard changes is unavailable for Mira
    And I am told to wait for saving to finish
    When the earlier entity save completes
    Then my newer entity edit remains visible as unsaved
    And Discard changes is available for Mira
    When I explicitly discard Mira's changes
    Then the version acknowledged by the completed save is restored

  Scenario: Retry a failed save with the keyboard
    Given a session save has failed while its title field is focused
    When I move to Retry and press Enter
    Then the session save is retried
    And focus returns to the session title

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
