# Chronacle User Guide

## Welcome to Chronacle

Chronacle is a helper app designed for Game Masters like you. It reads your rulebook PDFs, remembers what's in them, and answers your questions with page references — so you can spend less time flipping through books and more time running your game. Every piece of information stays on your computer. Nothing is shared, nothing is uploaded to the internet unless you choose to connect an online AI service.

---

## Quick Start

Here's how to go from nothing to asking your first question in a few minutes.

### 1. Download and open Chronacle

Install the app on your computer and launch it. You'll see the main window with a chat area on the left and a settings panel on the right.

### 2. Set up an AI Provider

Before Chronacle can answer questions, it needs a "brain" to do the thinking. Go to the **Settings** section and choose an AI service to connect.

The easiest way to start is with **OpenAI**. You'll need an **API Key** — think of it like a secret password that lets Chronacle talk to the OpenAI service.

- Go to [platform.openai.com](https://platform.openai.com) and create an account.
- Find the **API Keys** section and click **Create new secret key**.
- Copy the key (it looks like a long string of letters and numbers).
- Paste it into the OpenAI settings in Chronacle.

Don't worry — this key is stored only on your computer. See the **Setting Up an AI Provider** section below for other options, including free ones.

### 3. Upload a PDF rulebook

Click the **Upload** button and pick a PDF from your computer — try the Dungeon Master's Guide or any rulebook you use at the table. A progress bar will fill up as Chronacle reads the document and builds a searchable index. Depending on the size of the book, this can take anywhere from a few seconds to a minute or two.

You'll see the book appear in your library once it's ready.

### 4. Ask a question

Type a question into the chat box at the bottom — something like "What does the Slow spell do?" or "How does stealth work during combat?" — and press Enter.

Chronacle will search your rulebooks and write an answer. When it references a specific page, you'll see a citation like this:

> **[Source: Player's Handbook, p. 277]**

That's it. You're ready to go.

---

## Setting Up an AI Provider

An **AI Provider** is the service that actually answers your questions. Think of it as the brain behind Chronacle. You choose which brain to plug in.

Chronacle supports several options. Which one is right for you depends on whether you want to pay a small fee, use a free local option, or configure a custom service.

### Option 1: OpenAI (recommended for beginners)

**Cost:** Paid (usage-based, typically a few cents per session)  
**Internet required:** Yes  
**Difficulty:** Easy

OpenAI is the most popular choice. It's reliable, fast, and works right out of the box once you enter your API Key.

**To set it up:**

1. Go to [platform.openai.com](https://platform.openai.com) and sign up.
2. Click on your profile picture → **API Keys**.
3. Click **Create new secret key** and copy the key.
4. In Chronacle's Settings, choose **OpenAI** and paste your key.
5. Pick a **Model** from the list (starting with "gpt-4o" or "gpt-4o-mini" are great choices).

You're done. Chronacle will now use OpenAI to answer your questions.

### Option 2: Anthropic (alternative cloud provider)

**Cost:** Paid (usage-based)  
**Internet required:** Yes  
**Difficulty:** Easy

Anthropic is another AI provider. Some people prefer its style of answers. The setup is nearly identical to OpenAI.

**To set it up:**

1. Go to [console.anthropic.com](https://console.anthropic.com) and sign up.
2. Navigate to **API Keys** and create a new key.
3. Copy the key and paste it into Chronacle's Anthropic settings.
4. Pick a **Model** (starting with "claude-3" or "claude-3.5" is a good choice).

### Option 3: Ollama (free, runs on your computer)

**Cost:** Free  
**Internet required:** No (after installation)  
**Difficulty:** Medium

Ollama lets you run an AI directly on your own computer. No monthly fees, no internet connection needed — but your computer does the heavy lifting, so answers may be slower, especially on older machines. Writing can be a bit slower than cloud providers but it's completely private.

**To set it up:**

1. Download and install Ollama from [ollama.com](https://ollama.com).
2. Open Ollama and download a model. A good starting model is **llama3.2** or **mistral**. You can do this through Ollama's interface or by running this command in a terminal:
   ```
   ollama pull llama3.2
   ```
3. In Chronacle's Settings, choose **Ollama**.
4. Make sure the **Base URL** is set to `http://localhost:11434` (this is the default address Ollama uses on your computer).
5. Pick the model you downloaded from the list.

> 💡 **Tip:** Ollama is great if you're privacy-conscious or want to avoid per-usage costs. Just be aware that larger models need a powerful computer and can take a while to respond.

### Option 4: Custom Providers (for third-party services)

**Cost:** Varies by service  
**Internet required:** Yes (unless running locally)  
**Difficulty:** Medium

A **Custom Provider** is any AI service that speaks the same "language" as OpenAI or Anthropic. This includes services like **OpenRouter** (which gives you access to many models from one account), **Groq** (known for fast responses), or even a custom AI server you run yourself.

**To set one up:**

1. In Chronacle's Settings, go to **Custom Providers** and click **Add Provider**.
2. Give it a **Name** — something you'll recognize, like "OpenRouter" or "My Work Server".
3. Choose the **Compatibility** type:
   - **OpenAI-Compatible** — for most third-party services
   - **Anthropic-Compatible** — for services that follow Anthropic's format
4. Enter the **Base URL** — this is the web address of the service. For example:
   - OpenRouter: `https://openrouter.ai/api/v1`
   - Groq: `https://api.groq.com/openai/v1`
5. If the service requires an **API Key**, enter it here. Some local services don't need one.
6. Add one or more **Models** — each model needs:
   - **Model ID** — the internal name the service uses (like `gpt-4o` or `llama-3.1-70b`)
   - **Display Name** — what you want to call it in Chronacle (like "GPT-4o" or "Llama 3.1 70B")
7. Click **Save**.

Your custom provider will now appear in the provider list alongside OpenAI, Anthropic, and Ollama.

### Language and search

In **Settings**, choose **Automatic** to follow your operating-system language, or choose English,
German, French, or Spanish explicitly. An explicit choice takes effect immediately.

Oracle replies in a clearly detected English, German, French, or Spanish question. Short or
ambiguous questions use your interface-language setting instead. Your PDFs, entity names, rules,
and cited quotes always remain in their original language.

For search, the small local Nomic model keeps a smaller offline English-focused index. The local
multilingual E5 Base model is a larger offline download for German, French, Spanish, and
cross-language retrieval. Cloud embeddings also support multilingual retrieval, but require your
provider credentials. Whenever you change an embedding model, use **Settings → Re-index sources**
before existing sources can use that model.

---

## Loading Your Rulebooks

Now that you have an AI provider set up, it's time to load your books.

### How to upload a PDF

Click the **Upload PDF** button at the top of the screen. A file dialog will open — select one or more PDF files from your computer. Chronacle will start processing them one at a time.

You can choose which **campaign** a PDF belongs to before uploading:

1. Make sure the correct campaign is selected in the **campaign context** dropdown (visible below the chat input when a campaign exists).
2. Click **Upload PDF** — the PDF will be tagged with that campaign.
3. If no campaign is selected, the PDF is uploaded as a **Global Source**, available in all campaigns.

This is useful when you run multiple game systems or campaigns. For example, you might upload the Player's Handbook to a "Curse of Strahd" campaign and keep a different set of books for your "Homebrew World" campaign.

### What happens during ingestion

When you upload a PDF, Chronacle goes through a process called **Ingestion**. Here's what that means in plain terms:

1. **Read the pages** — Chronacle opens the PDF and reads the text on every page.
2. **Break it into pieces** — It splits the text into small, manageable chunks (think of it like cutting a long scroll into individual index cards).
3. **Index the pieces** — Each chunk is labeled with where it came from (book title, page number) and stored in a way that makes searching fast.

This is why you see a **progress bar** — the bar fills up as each step completes. A 300-page rulebook might take anywhere from 20 seconds to 2 minutes, depending on your computer's speed.

> 💡 **Tip:** You can keep using Chronacle while ingestion runs in the background. You don't need to stare at the progress bar — just let it do its thing.

### What kinds of PDFs work best?

Chronacle works with **text-based PDFs** — that is, PDFs where the text is already in digital form. Most rulebooks you buy from official publishers (Wizards of the Coast, Paizo, Free League, etc.) are text-based PDFs.

**Scanned PDFs** (also called "scanned images" or "image-only PDFs") don't work well. If your PDF is a set of scanned pages — like a photocopy of an out-of-print book — Chronacle can't read the text. If you try to upload one, you'll likely see an error. Unfortunately, converting scanned books requires special software that's beyond what Chronacle does.

> 💡 **Tip:** If you're unsure, just try uploading it. Chronacle will tell you if it can't read the file.

---

## Managing Campaigns

Campaigns let you organise your rulebooks by game. Each campaign can have its own set of PDF sources, and when you ask a question, Chronacle will search only the books belonging to that campaign (plus any global sources shared across all campaigns).

### What is a campaign?

A **campaign** is a container for your game's resources — rulebooks, notes, and chat history. You might create one campaign per actual table you run:

- "Curse of Strahd" — with the Player's Handbook, Curse of Strahd module, and Xanathar's Guide
- "Homebrew World" — with the Player's Handbook and Dungeon Master's Guide
- "Call of Cthulhu" — with the Call of Cthulhu rulebook and a campaign scenario

A **Global Source** is a PDF that isn't tied to any campaign. It's searchable from every campaign. The Player's Handbook often works well as a global source since it's useful in every game.

### The Campaigns page

Click the **Campaigns** button at the top to open the campaign manager. This page has two panels:

- **Left sidebar** — shows all your campaigns plus a "Global Sources" entry.
- **Main area** — shows all PDF sources belonging to the selected campaign.

### Creating a campaign

1. On the **Campaigns** page, click **+ New** in the sidebar.
2. Enter a **Campaign name** (e.g., "Curse of Strahd").
3. Enter the **Game system** (e.g., "D&D 5e" or "Call of Cthulhu 7e") — this is just for reference; it doesn't affect how Chronacle works.
4. Click **Create**.

The new campaign appears in the sidebar. You can now upload PDFs to it.

### Switching campaigns

On the **Chat** page, a **context selector** shows which campaign you're currently in:

> Context: **Global**
>
> [▼ Select campaign...]

- **Global** — search all global (non-campaign) sources. This is the default.
- **A specific campaign** — search that campaign's sources plus all global sources.

When you switch campaigns, the chat history reloads to show only messages from that campaign.

### Managing sources in a campaign

Click on any campaign (or "Global Sources") in the sidebar to see its PDFs.

Each source card shows:

- The **filename** and **display name**
- The **index status** — `pending`, `indexing`, `done`, or `error`
- The **page count**
- A **delete** button (appears on hover)

To **delete a source**, hover over its card and click the ✖ button. This removes the PDF, its index, and all associated chunks from Chronacle.

### Deleting a campaign

Hover over a campaign in the sidebar and click the ✖ button that appears. This permanently deletes the campaign and all its sources, indexed chunks, and messages. You'll be asked to confirm before the deletion happens.

> 💡 **Tip:** You can't undo deleting a campaign. Make sure you don't need the data anymore, or back up your Chronacle data directory first.

---

## Organising Your Library

The source list on the Campaigns page gives you a clear view of everything you've loaded. Each source card has a colored **status badge**:

- 🟢 **done** — The PDF was successfully indexed and is ready for questions.
- 🟡 **pending / indexing** — Chronacle is still processing this PDF. Wait a moment and refresh.
- 🔴 **error** — Something went wrong during ingestion. The PDF might be scanned (image-only) or corrupted.

If a source has an **error** status, try re-uploading it or check that the file is a text-based PDF.

---

## The Codex

Once you've loaded a book or two, Chronacle can go one step further than just answering questions on demand: it can **read through your library ahead of time** and write up a tidy, browsable summary of everything in it — a bit like an index card box that someone has already sorted and labeled for you. This is called **compiling the Codex**, and it happens per **collection** (the shelf of books grouped together in your campaign manager, like "World Guide" or "Monster Manual").

### What compiling does

When you click **Compile** on a collection, Chronacle reads through the new or changed material in that collection and writes short, focused articles about the people, places, and rules it finds — each one with page references back to the book it came from. This is genuinely useful work for the AI to do, and like every other time Chronacle asks the AI a question, **it costs a little bit of usage** with whichever AI Provider you've set up (the same kind of cost as asking a question in chat).

Because of that cost, Chronacle **never compiles automatically** — you decide when it's worth doing, usually after you've added a new book or made a batch of changes. If a collection has material waiting to be compiled (or re-compiled because something changed), you'll see a small badge like **"12 stale"** next to it. That number is just letting you know how much is waiting; nothing is out of date or broken, it simply hasn't been read yet. Click **Compile** whenever you're ready, and the badge will shrink as Chronacle works through it.

### Articles vs. your own notes

There are two very different kinds of writing living side by side in the Codex, and it's worth knowing which is which:

- **Articles** are written entirely by the AI when you compile. They're a helpful starting summary, but they are **not yours** — every time you recompile, the article for something that changed gets rewritten from scratch, and any edits you'd made directly to an article's text would be lost.
- **Notes** (and **table notes** — see below) are written entirely by **you**. Chronacle never touches them, never rewrites them, and never deletes them when you recompile. Anything you type in a notes field is permanently yours.

Think of articles as a first draft the AI hands you, and notes as the margin where you write down the truth as it actually plays out at your table.

### The Rules tab: seven kinds of rules

Inside a collection, the **Rules tab** shows every rule the Codex has compiled, sorted into seven categories. These categories are just a way of grouping similar rules together so you can find what you need quickly:

- **Mechanic** — a core rule for _how something works_, like how initiative is rolled or how advantage and disadvantage interact.
- **Ability** — a specific thing a character can do, like a spell, feat, or class feature (e.g. "Rage", "Fireball").
- **State** — a condition affecting a creature, like being "Poisoned" or "Prone."
- **Procedure** — a step-by-step process the table follows together, like running a chase scene or a long rest.
- **Resource** — something that gets **spent and regained**. Spell slots are a good example: you use them up during the day and get them back on a rest.
- **Statistic** — a number that **other rules read or change**. Armor Class is a good example: nobody "spends" AC, but plenty of rules check it or modify it.
- **Entry** — a catch-all for anything else worth remembering that doesn't fit neatly into the categories above.

The **resource vs. statistic** distinction trips people up the most, so here it is side by side: a **resource** is a tank you draw down and refill (spell slots, hit dice, ki points); a **statistic** is a fixed or slow-changing number that the rules point at (Armor Class, Difficulty Class, a saving throw target). If you can "run out" of it, it's probably a resource. If other rules compare against it, it's probably a statistic.

Click any rule's name to expand it and see its full write-up along with the page or pages it was compiled from, so you can always jump back to the source book if you want the exact wording.

### Table notes

Underneath every rule entry is a **table notes** box — a place for the house rules, clarifications, or reminders that are specific to _your_ table. Maybe you always round the damage of a certain spell up, or your group plays a condition slightly differently than written. Type it in the box, click away, and it's saved automatically — no save button to remember. As covered above, table notes are yours alone; recompiling the Codex will never touch them.

### Redo with objections

Sometimes the AI's write-up of a rule is wrong, incomplete, or just not how your table plays it. Rather than editing the article yourself (which would be overwritten the next time you compile), click **Redo with objections…** and tell the AI what's wrong in plain language — for example, "the range is wrong, it should be 60 feet, not 30." Chronacle sends that objection back to the AI and asks it to rewrite just that one entry.

The best part: this is cumulative. Every objection you've ever raised about that entry is kept and honored on every future redo, so once you've corrected something, it stays corrected — even after later recompiles pull in new source material.

### Saving answers and session notes

Compiling reads your rulebooks, but campaigns generate their own lore too — an NPC's backstory that came out mid-session, a new location the party stumbled into, a rule ruling you made on the fly. **Save to Codex** is how that kind of thing makes it into the Codex.

Under any assistant answer in chat, click **Save to Codex** and Chronacle reads that answer and drafts one or more **proposals**: a new article, an update to an existing entity, a new rule entry, and so on. The same distillation happens automatically at the end of a session for your session notes, turning what happened at the table into proposed Codex updates without you having to write it up twice.

Either way, **nothing changes in your Codex the moment you click Save**. Every proposal lands in your **Maintenance inbox** (the "Maintenance" item in the left rail) as pending, waiting for you to look at it. Open a proposal there and you'll see exactly what's being suggested — a side-by-side of the current text and the proposed text, plus the AI's reasoning for the change — and two buttons: **Accept**, which applies the change and folds it into the Codex, or **Reject**, which discards the suggestion and leaves everything exactly as it was.

The number badge next to **Maintenance** in the rail tells you how many things are waiting on you — pending proposals plus anything else flagged for review. An empty inbox means the Codex fully reflects the choices you've made; a badge just means there's something to glance at whenever you have a moment, not that anything is broken or urgent.

### Keeping the codex healthy

Proposals aren't the only thing that shows up in the Maintenance inbox. As your Codex grows — more articles, more entities, more cross-references between them — small inconsistencies naturally creep in, the same way they would in a hand-kept campaign binder. The **Findings** tab (next to Proposals, inside Maintenance) is where Chronacle surfaces those for you to look at, grouped by kind:

- **Wikilinks** — an article links to a name Chronacle can't resolve yet. If it looks like a variant spelling, Chronacle offers a suggested match; if it looks like something you haven't created yet, you can create the missing article from there. You can also dismiss the finding if you don't want to act on it now.
- **Possible duplicate** — two entities look like they might be the same thing (near-identical names, most often). Click **Merge** to fold them into one, keeping every relationship from both and turning the old name into an alternate name so existing links keep working; or **Open A** / **Open B** to compare first, or **Mark resolved** if they really are two different things. The **Names and duplicates** chapter below walks through what merging does.
- **Stale article** — an article was compiled from source material that's since changed (a book was re-ingested, or new pages were added to a collection) and hasn't been recompiled since. Click **Compile** to bring it up to date in place — this behaves exactly like compiling from the collection's Rules tab — or **Mark resolved** if you'd rather leave the older version as-is for now.
- **Scope violation** — a link crosses a boundary it shouldn't, most often a campaign-specific entity referencing something that belongs to a different campaign (or the reverse). Click **Delete edge** to remove just that cross-reference without touching either article, or **Mark resolved** if the link is intentional.
- **Orphaned edge** — a relationship between two entities survived after one side of it was deleted, leaving a dangling reference pointing at nothing. There's nothing to open here; **Mark resolved** clears it once you've confirmed it's safe to drop.

Findings don't appear out of nowhere — they're written when you compile, when proposals are accepted, and whenever you click **Check campaign** (visible at the top of the Findings tab), which runs a fresh pass over your active campaign's Codex and reports how many new findings turned up alongside how many are still open. None of this happens automatically in the background, so a quiet Findings tab doesn't mean everything is perfect — it means nothing has been checked recently. Run **Check campaign** any time you want a fresh read, especially after a big batch of edits or compiles.

Findings are advisory, not enforcement: nothing in Chronacle blocks you from leaving them unresolved, and an unresolved finding never breaks retrieval or chat. Think of the Findings tab the way you'd think of a proofreader's margin notes — worth a look, never a gate.

---

## Names and duplicates

**When the same thing has two names**

Your world is full of things that go by more than one name. The Free League and
the Free League. The Quassars and the Quassar Family. You know these are the
same, but Chronacle starts out taking every name literally — to it, "The Free
League" and "Free League" look like two different factions, and a link to
[[The Quassars]] doesn't find the Quassar Family at all.

You fix this by giving something **alternate names**. Open any entity and you'll
find an _Alternate names_ field. Anything you put there works exactly like the
entity's real name: links pointing at it land here, and Chronacle stops treating
it as a stranger. You only ever have to do this once per name — it sticks.

**Links that Chronacle sorts out by itself**

Most of the time you won't have to do anything. When you write a link that
doesn't match anything exactly, Chronacle looks for the obvious answer. If
there's exactly one thing it's clearly pointing at — [[The Quassars]] when the
Quassar Family is the only Quassar anything in your campaign — it makes the link
and remembers the name for next time.

It only does this when there's a single sensible answer. If two things could
both be what you meant, it won't guess: it asks.

Everything Chronacle links on its own shows up in **Maintenance** under
_Auto-linked_. You never have to look at that list — it's there so nothing
happens behind your back. If it ever gets one wrong, hit **Undo** and it will
ask you next time instead of deciding.

**Links Chronacle isn't sure about**

An unresolved link can also be intentional. You might write `[[Moon Gate]]`
before you've made a Moon Gate article, just to mark that it should exist later.
Chronacle treats that as a useful placeholder: when you click the unresolved
link in an article, or the matching missing node in the relationship graph, you
can choose what kind of article to create and start with the name already filled
in. If Chronacle also has a likely match, you can either use that suggestion or
create a separate new article instead.

**Merging two entries that are the same thing**

If you've ended up with two entries for one thing — it happens easily when a
rulebook says "the Free League" and your session notes say "Free League" —
Chronacle will spot it and offer to merge them.

You'll see them side by side. Pick which one to keep, and for each piece of
writing — the summary, your notes — choose which version survives, or keep both.
Relationships are always kept from both sides: if one entry knew about a
connection the other didn't, the merged entry knows about it too. Nothing gets
quietly dropped.

The name of the entry you didn't keep isn't lost either — it becomes one of the
merged entry's alternate names. Every link you ever wrote using it keeps working.

The merged entry's codex article is marked for rewriting, because it was written
from half the facts. Recompile when you're ready.

---

## Asking Questions

Once your rulebooks are loaded, the chat area is your main tool. Think of it like messaging a very knowledgeable assistant who has read every book you've loaded.

### How to ask

Type your question in the text box at the bottom and press Enter. Before you ask, check the **context selector** above the input — it shows which campaign's sources Chronacle will search:

- **Global** — searches only global (non-campaign) sources.
- **A specific campaign** — searches that campaign's sources plus global sources.

You can ask anything that you'd normally look up in a rulebook:

- "What are the requirements for casting a ritual spell?"
- "How does the Rogue's Sneak Attack work?"
- "List all the cantrips available to Wizards in the Player's Handbook"
- "Explain how cover works in ranged combat"

### The "Thinking…" state

After you ask a question, Chronacle will show a "Thinking…" indicator. This means the app is doing two things:

1. Searching through your rulebooks to find relevant passages.
2. Sending those passages to the AI to compose an answer.

Depending on the AI provider you chose and the complexity of your question, this might take anywhere from a few seconds to about 30 seconds. Cloud-based providers (OpenAI, Anthropic) tend to be faster. Local providers (Ollama) may be slower.

> 💡 **Tip:** If "Thinking…" takes longer than a minute, you can try simplifying your question or checking your internet connection (for cloud providers).

### How citations work

When the AI finds information in your rulebooks, it will tell you exactly where it came from. You'll see citations like:

> The Slow spell affects up to 6 creatures of your choice within a 40-foot cube. Each target must make a Wisdom saving throw or be slowed for the duration.
>
> _[Source: Player's Handbook, p. 277]_

The citation includes the **book title** and the **page number**. If the information was found across multiple pages, you might see something like:

> _[Source: Dungeon Master's Guide, pp. 45–47]_

Occasionally the response might mix information from multiple sources:

> _[Sources: Player's Handbook, p. 192; Xanathar's Guide to Everything, p. 56]_

### What if the AI doesn't know the answer?

AI isn't perfect. Sometimes it won't find what you're looking for, or it might give a general answer without a citation. Here's what to try:

- **Rephrase your question.** Try using different words. Instead of "How does hiding work?", try "What are the rules for hiding during combat?"
- **Be more specific.** "What level is the Fireball spell?" is easier to answer than "Tell me about all fire spells."
- **Check that the PDF was loaded.** Make sure the book you're thinking of actually appeared in your library after uploading.

If the AI gives an answer **without a citation**, it might be making a guess based on general knowledge rather than your rulebooks. Always double-check rules that don't have a source citation.

---

## Your Vault

Chronacle can mirror a campaign into an ordinary folder of text files on your computer — the same kind of folder [Obsidian](https://obsidian.md) reads, but it works with any text editor, not just Obsidian. This section explains what that folder is, how it stays in step with Chronacle, and how to handle it if you and Chronacle change the same thing at the same time.

### What vault sync is

Every NPC, location, faction, creature, item, event, and player character in a campaign — plus your session write-ups and the Codex's compiled rule entries — can be written out as a `.md` file. Point Chronacle at a folder (an empty one, or a live Obsidian vault you already use) and it fills that folder with one file per record, organised into subfolders by campaign or collection.

This isn't a one-time export. It's a two-way street:

- **Chronacle → your files.** Whenever something changes in Chronacle — you edit an NPC, save an answer to the Codex, or compile a collection — the matching file is rewritten within a couple of seconds, automatically, with no button to press.
- **Your files → Chronacle.** Open a file in Obsidian (or Notepad, or anything else) and edit it. Chronacle notices the change and pulls it back in — again within a couple of seconds, no button needed.
- **"Sync now."** You don't have to rely on the automatic watch. **Settings → Markdown vault → Sync now** runs a full pass in both directions on demand — handy right after you've made a batch of edits, or if you're not sure everything caught up.

**To set it up:** open **Settings**, find the **Markdown vault** panel, and click **Choose folder…**. Pick any folder — a new empty one, or the root of an Obsidian vault you already have open. Chronacle writes your campaign into it immediately. To stop syncing, click **Disconnect** — this does not delete anything, it just stops watching.

### What's yours vs Chronacle's in a file

Open any entity's `.md` file and you'll find three regions. Here's what a real NPC file looks like — Seraphina Aldric, the archivist of the Iron Tower, in the "Shadows of Valdris" campaign:

```text
---
id: "npc:abc123"
name: "Seraphina Aldric"
title: "Seraphina Aldric"
aliases: ["Seraphina Aldric"]
type: "npc"
campaign: "Shadows of Valdris"
created_at: "2026-05-28T14:00:00Z"
updated_at: "2026-07-09T18:32:00Z"
---

## Summary

Archivist of the Iron Tower.

<!-- chronacle:codex-article start -- compiled; edits are not applied -->
Seraphina is the archivist of [[The Iron Tower]].
<!-- chronacle:codex-article end -->

## Notes

GM notes.
```

Three things are happening in that file:

- **The block between the `---` lines at the very top** is Chronacle's bookkeeping — the record's ID, its name, what campaign it belongs to, and timestamps. Chronacle rewrites this block every time it syncs. Don't edit it, and in particular never touch or remove the `id` line — that's the only thing that tells Chronacle "this file is Seraphina Aldric" rather than a new, different NPC.
- **The block between `<!-- chronacle:codex-article start -- compiled; edits are not applied -->` and `<!-- chronacle:codex-article end -->`** is the AI-compiled article — the same text you'd see on Seraphina's Codex entry. Exactly as the comment says, edits inside this fence are not applied: if you type over it, your text is overwritten the next time Chronacle syncs. If you want a compiled article to say something different, use **Redo with objections…** in the Codex (see **The Codex** above) rather than editing the file directly.
- **Everything else — the `## Summary` line, the `## Notes` section, and any other text you add** — is yours. Type in the Summary field, write freeform notes under `## Notes`, add your own headings, whatever you like: it all flows back into Chronacle on the next sync, into the matching Summary and Notes fields on that entity.

Session files and compiled rule entries follow the same split, minus whichever pieces don't apply to them: a session file has no compiled fence (there's no AI-written article for a session), so everything below its frontmatter is yours. A rule entry's body is the compiled fence, same as an article.

### Alternate names in your vault files

Each file has an `aliases:` line near the top. That's the entity's alternate names,
and you can edit it in Obsidian directly — add one and Chronacle picks it up on
the next sync, exactly as if you'd typed it into the app. Obsidian uses the same
line for its own linking, so a name you add here works in both places at once.

Leave the entity's own name in the list. It's what makes your `[[links]]` in
Obsidian find the file.

### Conflicts

A conflict happens when the same record changes in **both** places between syncs — say, you rewrite Seraphina's `## Notes` in Obsidian on your laptop at the table, while a player conversation earlier in Chronacle triggered a Codex update to the same entity before that sync ran. Chronacle can't safely guess which version you want, so it doesn't try to merge them automatically. Instead:

1. Chronacle writes its own version of the file next to yours, named `seraphina-aldric.conflict.md` — your file, `seraphina-aldric.md`, is left completely untouched.
2. That record **freezes**: Chronacle stops syncing it in either direction until you resolve the conflict, so neither version can silently overwrite the other.
3. You'll see it listed in **Settings → Markdown vault**, under **Conflicts**, showing the record's name and both file paths. If you open the record itself in Chronacle, you'll also see a banner: "This record has unsynced vault edits in conflict — resolve in your vault ({file path})."

**To resolve it:**

1. Open both files side by side — `seraphina-aldric.md` (yours) and `seraphina-aldric.conflict.md` (Chronacle's).
2. Copy across whatever you want to keep from the `.conflict.md` file into your own file, `seraphina-aldric.md`.
3. Delete `seraphina-aldric.conflict.md`.
4. On the next sync (automatic, or **Sync now**), Chronacle sees the sidecar is gone, treats that as your decision, and applies the content of your file. The record unfreezes and syncs normally again.

If your file's top metadata block got damaged or deleted while you were editing, Chronacle can't read it as a valid record — in that case it puts the `.conflict.md` sidecar back instead of losing its version, so the record stays visibly in conflict rather than vanishing into a stuck state. Fix the metadata block (or restore it from the sidecar's own copy) and try deleting the sidecar again.

### Deleting

Deleting works differently depending on which side you delete from:

- **Delete the file in your vault** (in Obsidian, in Finder, wherever) → the record disappears from Chronacle. It isn't destroyed — it's hidden, the same way a soft-deleted record is hidden everywhere else in the app.
- **Delete the record inside Chronacle** — the **Delete** button on an entity's card in the entity manager, after confirming "Remove Seraphina Aldric? It disappears from Chronacle and your vault." → its vault file is removed too, **unless** you had hand-edited that file since Chronacle last wrote it. In that case Chronacle leaves your edited file alone rather than throwing away work you haven't synced yet.

### Switching folders

If you click **Choose folder…** again and pick a different folder, Chronacle re-exports your whole campaign into the new location. **Nothing is deleted** — the old folder is simply left as-is once Chronacle stops watching it, and files in the new folder aren't cleared out first. The Markdown vault panel reminds you of this: "Changing the folder re-exports everything; nothing is deleted." Pick an empty folder (or a fresh spot inside your Obsidian vault) if you want a clean re-export with nothing left over to sort out.

---

## Chat History

Every conversation you have with Chronacle is saved automatically. When you close the app and open it again, your previous chats will be waiting for you.

- **Chat history is per-campaign.** When you switch campaigns using the context selector, Chronacle loads only that campaign's messages. Switching back restores the previous campaign's messages.
- **Your conversations stay on your computer.** They are not sent anywhere or shared with anyone.
- **You can start a new chat** at any time by switching to a different campaign.

> 💡 **Tip:** Global messages (with no campaign selected) are shared across all campaigns. Use campaign-specific chat to keep different games' conversations separate.

---

## Troubleshooting

### "I see only a dark screen"

The app is still loading. On slower computers, Chronacle may take a few seconds to start up. If the screen stays dark for more than 30 seconds, try closing and reopening the app.

### "Chat says 'No LLM provider configured'"

You haven't set up an AI Provider yet. Go to **Settings** and configure one of the options described in the **Setting Up an AI Provider** section above. Even a free local option like Ollama works.

### "Upload says 'Error'"

The PDF you tried to upload might be a scanned document (images only, no readable text), or the file might be damaged. Try:

- Uploading a different PDF to see if the problem is with the file.
- Checking that the PDF isn't password-protected or encrypted.
- Making sure the file isn't still open in another program.

If you continue to see errors, the PDF may be a scanned image. See **Loading Your Rulebooks** above for more on what kinds of PDFs work best.

### "Response doesn't cite sources"

The AI might be answering from general knowledge rather than your rulebooks. Try:

- Rephrasing your question with more specific wording.
- Including the name of the book in your question: "What does the Player's Handbook say about grappling?"
- Making sure the relevant book was uploaded successfully.

If the AI consistently answers without citations, it may mean the search didn't find matching passages in your loaded PDFs. Try asking a simpler question that you know is in one of your books to verify.

### "The AI isn't answering / 'Thinking…' never finishes"

This usually means the AI provider can't be reached. Check:

- **For cloud providers (OpenAI, Anthropic):** Is your computer connected to the internet? Is your API Key correct? Check the Settings to make sure you didn't accidentally delete or change the key.
- **For Ollama:** Is Ollama running on your computer? Did you download a model? Make sure the Base URL in Settings is `http://localhost:11434`.

### "I got a blank or confusing response"

Occasionally the AI might give a strange or incomplete answer. This is rare but can happen. Try:

- Asking the question again in different words.
- Starting a new chat and asking a simpler question first.
- Checking if your API Key has run out of credits (for paid services like OpenAI or Anthropic).

---

## Glossary

This glossary explains the technical terms used in this guide. If you see a word you don't recognize, check here first.

**AI Provider** — The service or program that does the "thinking" when you ask a question. It's the brain behind Chronacle. Examples: OpenAI, Anthropic, Ollama. (Also called an **LLM** — see below.)

**LLM (Large Language Model)** — A fancy name for the AI brain that reads your question, looks at your rulebooks, and writes an answer. "Large Language Model" just means it's a computer program trained on a vast amount of text. You don't need to remember the acronym — just know that this is what answers your questions.

**API Key** — A secret password that lets Chronacle talk to an online AI service like OpenAI or Anthropic. It's a long string of letters and numbers. Think of it like a key to a locked door — keep it private, don't share it with others. Chronacle stores it safely on your computer.

**Model** — A specific version or flavor of an AI. Different models have different strengths, speeds, and costs. Examples: `gpt-4o` (OpenAI), `claude-3-haiku` (Anthropic), `llama3.2` (Ollama). When you set up a provider, you'll pick which model to use.

**Token** — A small piece of a word that the AI reads and writes in. For example, the word "fireball" might be broken into two tokens: "fire" and "ball". The AI doesn't read whole words at once — it reads tokens. You don't need to worry about tokens day-to-day, but they're how AI services measure how much text they're processing.

**Ingestion** — The process of reading a PDF, breaking it into small pieces, and organizing those pieces so Chronacle can search through them quickly. Think of it like taking a thick rulebook, cutting each page into paragraphs, putting each paragraph on an index card, and filing those cards in order.

**Embedding** — A numeric "fingerprint" that Chronacle creates for each piece of text (each "index card") in your rulebooks. When you ask a question, Chronacle turns your question into the same kind of fingerprint and looks for the closest matches in its collection. You don't need to understand how it works — just that it's how the app finds the right passages.

**Vector Search** — The method Chronacle uses to find the most relevant passages from your rulebooks. Think of it like a filing cabinet where every index card is placed near similar cards. When you ask a question, Chronacle pulls out the cards that are closest to your question. It's called "vector" because each piece of text is treated as a point in a giant map, and the search finds the nearest points.

**Citation** — A reference that tells you exactly which book and page the AI used to answer your question. It looks like `[Source: Player's Handbook, p. 277]`. Citations help you verify the answer and find the original text if you want to read more.

**OpenAI-Compatible** — A description for AI services that use the same technical "language" as OpenAI. This means they work with Chronacle the same way OpenAI does, even if they're run by a different company. Services like OpenRouter and Groq are OpenAI-compatible.

**Custom Provider** — An AI service that you configure yourself in Chronacle, rather than using one of the built-in options (OpenAI, Anthropic, Ollama). You give it a name, enter its web address, and tell it which models to use. This is useful for third-party services and self-hosted AI servers.

**Base URL** — The web address where an AI service lives. For example, Ollama's base URL is `http://localhost:11434`, which means it's running on your own computer. For a cloud service, it would be something like `https://api.openai.com/v1`. You'll need to enter this when setting up a Custom Provider.

**Campaign** — A container for your game's resources: rulebooks, chat history, and notes. Each campaign has its own set of PDF sources and its own conversation history. You can switch between campaigns to keep different games separate.

**Global Source** — A PDF that isn't tied to any campaign. It's searchable from every campaign, making it useful for core rulebooks like the Player's Handbook that you use across all your games.

**Markdown vault** — A folder of ordinary `.md` text files that Chronacle keeps in step with a campaign, so you can browse and edit your NPCs, locations, and sessions in Obsidian or any text editor. See **Your Vault** above.

**Conflict (vault sync)** — What happens when the same record is edited both in Chronacle and in a vault file before the two get a chance to sync. Chronacle saves its own version as a `.conflict.md` file next to yours and pauses syncing that record until you resolve it. See **Your Vault → Conflicts** above.

---

_Chronacle keeps your game running smoothly — so you can focus on what matters: telling great stories with your players._
