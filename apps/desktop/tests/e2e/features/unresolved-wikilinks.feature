Feature: Unresolved wikilink creation

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app

  Scenario: Create a missing article from a clicked wikilink
    Given an NPC article contains the unresolved link "[[Moon Gate]]"
    When the GM clicks the unresolved link "[[Moon Gate]]"
    And creates a Location named "Moon Gate"
    Then the create command is sent for a Location named "Moon Gate"

  Scenario: Choose between a suggestion and a new article in Maintenance
    Given Maintenance has a wikilink finding for "[[Moon Gat]]" with a suggestion "Moon Gate"
    When the GM opens the finding
    Then they can use the suggestion
    And they can instead create a new article named "Moon Gat"

  Scenario: Treat no-candidate wikilinks as missing articles
    Given Maintenance has a wikilink finding for "[[Ashen Ferry]]" with no candidates
    When the GM opens the finding
    Then the finding is labeled "Missing article"
    And the primary action is "Create article"

  Scenario: Create a missing article from the relationship graph
    Given an NPC article contains the unresolved link "[[Moon Gate]]"
    When the GM opens that NPC's relationship graph
    Then the graph shows a distinct missing-link node named "[[Moon Gate]]"
    When the GM clicks the missing-link node
    And creates a Location named "Moon Gate"
    Then the create command is sent for a Location named "Moon Gate"
