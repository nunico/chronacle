---
translationKey: codex.health
locale: en
slug: codex/maintenance
title: Maintain the Codex
summary: Review proposed changes and advisory findings before deciding what belongs in your campaign record.
section: codex
order: 7
headings:
  - id: review-proposals
    text: Review proposals
    level: 2
  - id: check-findings
    text: Check findings
    level: 2
  - id: example
    text: Example
    level: 2
  - id: safe-consequences
    text: Safe consequences
    level: 2
---

Maintenance is an inbox for pending Codex proposals and findings that deserve your review.

<h2 id="review-proposals">Review proposals</h2>

Open **Maintenance → Proposals**. Each card shows its kind and origin, the current and proposed text, and a rationale. **Accept** applies it; **Reject** leaves the target unchanged and removes the pending proposal.

<h2 id="check-findings">Check findings</h2>

Choose **Findings**, then **Check campaign** to scan the active campaign. Findings can cover unresolved wiki links, stale articles, possible duplicates, naming conflicts, scope violations, orphaned relationship edges, and reviewable automatic links. Use the action on each card or **Dismiss** when no change is needed.

<h2 id="example">Example</h2>

Under an answer about **Mara Venn**, choose **Save to Codex**. Chronacle may find nothing worth saving or may create one or more proposals of different kinds. If it creates a **Notes suggestion** saying “Mara owes the crew one moonrise crossing,” compare it with the current note and accept it only if that became true in play. Then run **Check campaign** and review any resulting **Stale article**.

<h2 id="safe-consequences">Safe consequences</h2>

- **Delete edge** removes only the flagged relationship, not either entity.
- **Compile** on a stale finding makes one compile attempt, then clears the Maintenance item when the command finishes—even when no source context was found and the article stayed unchanged. Inspect the article afterward. If it did not change, restore access to the relevant source material and compile again through the collection's normal **Compile** action or the entity's **Recompile article** action.
- **Dismiss** resolves the finding without making its suggested content change.
- A quiet inbox means no pending items are listed; it is not proof that every article is correct.
