Feature: Alternate names and duplicates
  A GM can confirm a suggested alternate name and merge duplicate entities
  without losing data (ADR-011).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app

  Scenario: Confirming a suggested alternate name fixes the link
    Given the maintenance inbox has a broken-wikilink finding for "[[The Quassars]]" that could mean "The Quassar Family"
    When the GM opens the findings tab
    Then the finding "Broken wikilink" is listed with "The Quassar Family"
    When the GM confirms the suggestion "The Quassar Family"
    Then the confirm-alternate-name command is sent for that entity and alias

  Scenario: Merging two entities keeps every relationship
    Given the maintenance inbox has a duplicate-entity finding for "The Free League" and "Free League"
    When the GM opens the findings tab
    And the GM clicks "Merge" on the duplicate finding
    And the GM keeps "The Free League" as the survivor and confirms the merge
    Then the merge command is sent with the survivor and loser entities
