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
  - id: example
    text: Example
    level: 2
  - id: current-limits
    text: Current limits
    level: 2
---

The **Sessions** view keeps a numbered, campaign-specific log whose title, date, and notes save when you leave each field.

<h2 id="record-a-session">Record a session</h2>

1. Select a campaign and open **Sessions**.
2. Choose **New session**. Chronacle uses the next session number, prefills a date from the computer's UTC date, and adds a title such as **Session 4**. Check and adjust the date to the local date you played.
3. Expand the row and edit **Name**, **Date played**, and **Notes**.
4. Use `[[Entity Name]]` in the recap. Click outside a field to save it.

<h2 id="expected-result">Expected result</h2>

Sessions are shown by session number, lowest first. Their text is stored with the campaign and can inform later campaign questions. Saving non-empty notes can also draft proposals in **Maintenance** for you to accept or reject.

<h2 id="example">Example</h2>

Create **Session 6 — Fire at North Quay**, dated 2026-08-15, with notes:

> [[Mara Venn]] ferried the crew away. [[Iria Pell]] recovered the brass bell. Next: find who lit the warehouse.

Later, ask “What remained unresolved after the North Quay fire?” and check the response against the saved recap.

<h2 id="current-limits">Current limits</h2>

- The session number is assigned on creation and is not editable in the current form.
- The event count shows events explicitly assigned to that session, not every wiki-linked entity in the notes.
- **Delete** permanently removes the session after confirmation.
