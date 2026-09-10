---
translationKey: notes.sessions
locale: en
slug: notes-and-sessions/session-log
title: Keep a session log
summary: Record each session's title, played date, linked events, and recap in campaign order.
section: notes-and-sessions
order: 2
headings:
  - id: record-a-session
    text: Record a session
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: watch-the-save-status
    text: Watch the save status
    level: 2
  - id: recover-a-new-session
    text: Recover a new session
    level: 2
  - id: example
    text: Example
    level: 2
  - id: current-limits
    text: Current limits
    level: 2
---

The **Sessions** view keeps a numbered, campaign-specific log whose title, date, and notes save automatically after an ordinary focus change within that session.

<h2 id="record-a-session">Record a session</h2>

1. Select a campaign and open **Sessions**.
2. Choose **New session**. Chronacle uses the next session number, prefills a date from the computer's UTC date, and adds a title such as **Session 4**. Check and adjust the date to the local date you played.
3. Expand the row and edit **Name**, **Date played**, and **Notes**.
4. Use `[[Entity Name]]` in the recap. Move focus to another control in the same session to request a save.

<h2 id="expected-result">Expected result</h2>

Sessions are shown by session number, lowest first. Their text is stored with the campaign and can inform later campaign questions. Saving non-empty notes can also draft proposals in **Maintenance** for you to accept or reject.

<h2 id="watch-the-save-status">Watch the save status</h2>

Moving focus to another control in the same session requests an automatic save. Moving directly to another session or view retains the edit as **Unsaved changes** without treating navigation as a save. **Saving…**, **Saved**, and **Couldn't save** show the rest of its progress. If saving fails, your text remains in that session and **Retry** repeats the save to the same record. You can continue editing while a save is running; an earlier completion does not mark newer text as saved. See [Keep drafts and recover saves](/en/manual/notes-and-sessions/saving-and-recovery) for navigation and close behavior.

<h2 id="recover-a-new-session">Recover a new session</h2>

If **New session** cannot create the session, the failed attempt stays with the campaign where you started it. **Retry** uses the same intended session number, title, date, and notes. Repeated activation cannot start overlapping creation or create a duplicate session. **Discard changes** abandons only that failed attempt.

If the session was created but Chronacle shows **Created, but needs attention**, the listed saved session already has changes that must be settled first. Save or discard those conflicting changes, then choose **Retry** in the alert. That Retry finishes connecting the retained attempt to the session that was already created; it does not create another session.

<h2 id="example">Example</h2>

Create **Session 6 — Fire at North Quay**, dated 2026-08-15, with notes:

> [[Mara Venn]] ferried the crew away. [[Iria Pell]] recovered the brass bell. Next: find who lit the warehouse.

Later, ask “What remained unresolved after the North Quay fire?” and check the response against the saved recap.

<h2 id="current-limits">Current limits</h2>

- The session number is assigned on creation and is not editable in the current form.
- The event count shows events explicitly assigned to that session, not every wiki-linked entity in the notes.
- **Delete** permanently removes the session after confirmation.
