Feature: Preserve entity drafts while moving through a campaign
  A GM can move among Oracle, notes, and reference material without losing work,
  and can tell whether each edit is pending, saved, or needs attention.

  Background:
    Given draft reliability test data is available
    And I have opened Chronacle in campaign "Campaign A"

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

  Scenario: Ignore an entity list that finishes after a newer save acknowledgment
    Given saving my changed entity "Mira" is held in progress
    And the next NPC list load is held with Mira's earlier saved content
    When I navigate away and return to NPCs
    And the held save completes with its canonical content
    And the pre-acknowledgment entity list completes
    And I open entity "Mira Moonshadow"
    Then the acknowledged canonical name and notes are shown in the row, preview, and editor
    And the acknowledged entity is shown as saved
    And Mira's canonical changes remain persisted in campaign A's NPC record

  Scenario: Keep an acknowledged entity omitted by an older list
    Given saving my changed entity "Mira" is held in progress
    And the next NPC list load is held without Mira's earlier row
    When I navigate away and return to NPCs
    And the held save completes with its canonical content
    And the pre-acknowledgment entity list completes
    And I open entity "Mira Moonshadow"
    Then the acknowledged canonical name and notes are shown in the row, preview, and editor
    And the acknowledged entity is shown as saved
    And Mira's canonical changes remain persisted in campaign A's NPC record

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

  Scenario: Keep a promoted creation omitted by an older entity list
    Given creation of revision 1 for a new NPC is in progress
    When I navigate to Oracle
    And I hold the next NPC list before the assigned ID exists
    And I return to NPCs while that older list waits
    Then the new NPC draft is restored with Create unavailable
    When creation completes and assigns the NPC an ID
    And the pre-acknowledgment entity list completes
    Then the acknowledged NPC remains visible and selected
    And the interface indicates that it is saved
    And the explicit action is now Save
    And the retained NPC is not offered backend-only row actions
    And the created NPC remains persisted in campaign A's NPC records
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

  Scenario: Delete the entity captured by confirmation
    Given NPC "Mira" and location "Mira" both exist
    When I open deletion confirmation for NPC "Mira"
    And I try to navigate to locations with the keyboard
    And I confirm the captured entity deletion
    Then the NPC "Mira" is deleted
    And the location "Mira" remains
    And only the NPC draft is removed

  Scenario: Wait for an entity save before deletion
    Given a save for NPC "Mira" is in progress
    When I try to delete NPC "Mira"
    Then entity deletion is unavailable
    And I am told to wait for entity saving to finish
    When the entity save succeeds
    And I delete NPC "Mira"
    Then the entity and only its retained draft are removed

  Scenario: Retain an entity draft during keyboard navigation
    Given I have changed the notes for entity "Mira" without saving
    When I navigate to Oracle with the slash shortcut
    And I return to NPCs with the g chord
    Then my entity edits are preserved
    And the interface indicates that they are not yet saved
    And no entity save has been sent
