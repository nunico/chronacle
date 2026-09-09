---
translationKey: notes.saving-recovery
locale: en
slug: notes-and-sessions/saving-and-recovery
title: Keep drafts and recover saves
summary: Move between campaign work safely, understand save status, retry failures, and close Chronacle without silently losing a draft.
section: notes-and-sessions
order: 6
headings:
  - id: read-the-save-status
    text: Read the save status
    level: 2
  - id: move-without-losing-a-draft
    text: Move without losing a draft
    level: 2
  - id: save-or-discard-an-entity
    text: Save or discard an entity
    level: 2
  - id: recover-an-automatic-save
    text: Recover an automatic save
    level: 2
  - id: close-chronacle-safely
    text: Close Chronacle safely
    level: 2
  - id: know-the-recovery-limit
    text: Know the recovery limit
    level: 2
---

Chronacle keeps unfinished questions and edits attached to the campaign and record where you entered them. Navigation itself does not submit, save, or discard that work.

<h2 id="read-the-save-status">Read the save status</h2>

- **Unsaved changes** means Chronacle is retaining a newer version that the database has not acknowledged.
- **Saving…** means a write is in progress. You can keep typing; a completion for older text cannot mark newer text as saved.
- **Saved** means the applicable version was acknowledged. It does not describe an unsent Oracle question.
- **Couldn't save** keeps the text and the error visible. Choose **Retry** to write the latest retained version to the same record.

If you edit back to the known saved value, **Unsaved changes** clears without another write. Rapid requests to save the same record are handled in order. Chronacle does not run overlapping writes that could let an older value replace a newer one.

<h2 id="move-without-losing-a-draft">Move without losing a draft</h2>

- An unsent Oracle question remains when you visit a notebook or another view and return. Each campaign—and **No campaign**—has a separate composer. Returning never sends it.
- Existing and new entity drafts remain when you change views, notebook records, or campaigns. Reopening the same campaign and record restores its draft and status.
- Session-field and rule-note edits remain attached to their exact campaign and record while you move elsewhere, including while a save is pending.

Drafts never appear in another campaign or another record.

<h2 id="save-or-discard-an-entity">Save or discard an entity</h2>

Entity forms remain explicit: choose **Create** for a new entity or **Save** for an existing one. Simply starting a new entity or navigating away does not create a record. To abandon an entity draft, choose **Cancel**, then confirm **Discard changes**; unrelated drafts remain intact. If an entity write is already active, discard is unavailable until it finishes because that write cannot be taken back.

If a Create succeeds but Chronacle finds a newer version of the returned record, it shows **Created, but needs attention**. Choose **Keep saved record** or **Keep my draft**. Neither choice starts another backend save; choose **Save** afterward if you keep a draft that still has unsaved changes.

If the target was deleted or is otherwise unavailable, the failed draft remains visible as unavailable. **Retry** still targets that record and never redirects your text elsewhere. Use **Discard changes** when you no longer need the draft.

<h2 id="recover-an-automatic-save">Recover an automatic save</h2>

Session title, played date, and notes, and compiled-rule **Table notes**, request a save when focus moves to another control within the same session or rule. Moving directly to another record or view retains the draft as **Unsaved changes** without starting a save. If a save request fails:

1. Keep the displayed text; it has not been replaced by the saved version.
2. Read the inline error beside that editor.
3. Correct the content or connection problem if the message identifies one.
4. Choose **Retry**. A successful acknowledgment clears the error and shows **Saved**.

Retry keeps the original campaign and target. Multiple quick edits are reduced to the latest requested version without concurrent writes.

<h2 id="close-chronacle-safely">Close Chronacle safely</h2>

Closing the Chronacle window normally is blocked while the app holds an unsent question, an unsaved or failed edit, or an active save. The **Unsaved changes** dialog starts on the safe **Cancel** action:

- Choose **Cancel** or press Escape to keep working. Focus returns to where you were.
- With no write in progress, choose **Discard and close** only if you intend to remove every retained draft and close. Closing does not start a last-second save.
- While a write is already active, **Discard and close** is unavailable until it finishes. If all work then becomes saved, the dialog reports that it is safe to close and offers **Close**. If risk remains, you can cancel or explicitly choose **Discard and close**.

<h2 id="know-the-recovery-limit">Know the recovery limit</h2>

Chronacle keeps these drafts only in memory for the current run. A crash, power loss, forced termination, or restart cannot restore them. Private campaign draft text is not copied into browser storage or another durable draft store. Complete important saves and resolve the close dialog before ending the app.
