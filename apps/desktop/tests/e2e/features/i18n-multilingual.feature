Feature: Interface and Oracle language selection
Scenario: The saved interface language overrides the operating system
Given the operating system locale is "en-US"
And the saved interface language is "de"
When Chronacle opens Settings
Then the Settings heading is "Einstellungen"

Scenario: A supported message language takes precedence for Oracle
Given the saved interface language is "de"
When I ask Oracle "Quelle est la règle pour le grappin ?"
Then the Oracle request response language is "fr"

Scenario: Switching embedding modes requires re-indexing
Given sources were indexed with "nomic-embed-text-v1.5"
When I select the local multilingual embedding mode
Then Chronacle shows that source embeddings require re-indexing
