Feature: Compiled rules browsing
  Compiled rules are browsable per collection, grouped by category, with
  GM-owned table notes and redo-with-objections (ADR-009).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app
    And the GM opens the campaign manager
    And the GM opens the rules tab of collection "World Guide"

  Scenario: Rule entries are grouped by category with page references
    Then the rules list shows "Initiative" under the "mechanic" category
    And the entry "Initiative" cites "Core Rulebook p.12-13"

  Scenario: The GM disputes a rule with an objection
    When the GM opens the rule entry "Initiative"
    And the GM submits the objection "the range is wrong"
    Then a redo command is sent for the entry "Initiative"
