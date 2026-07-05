Feature: Codex write-back review
  Durable results reach the codex only through reviewed proposals (ADR-009).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app

  Scenario: Saving an answer to the codex creates reviewable proposals
    Given the assistant has answered a question
    When the GM clicks "Save to Codex" on the answer
    Then the save-to-codex command is sent with the answer text
    And a toast reports the created proposals

  Scenario: Accepting a proposal applies it and rejecting changes nothing
    Given the maintenance inbox lists a pending proposal for "Mira"
    When the GM accepts the proposal
    Then the accept command is sent for that proposal
    When the GM rejects the remaining proposal
    Then the reject command is sent for that proposal
