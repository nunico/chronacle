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
