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
> *[Source: Player's Handbook, p. 277]*

The citation includes the **book title** and the **page number**. If the information was found across multiple pages, you might see something like:

> *[Source: Dungeon Master's Guide, pp. 45–47]*

Occasionally the response might mix information from multiple sources:

> *[Sources: Player's Handbook, p. 192; Xanathar's Guide to Everything, p. 56]*

### What if the AI doesn't know the answer?

AI isn't perfect. Sometimes it won't find what you're looking for, or it might give a general answer without a citation. Here's what to try:

- **Rephrase your question.** Try using different words. Instead of "How does hiding work?", try "What are the rules for hiding during combat?"
- **Be more specific.** "What level is the Fireball spell?" is easier to answer than "Tell me about all fire spells."
- **Check that the PDF was loaded.** Make sure the book you're thinking of actually appeared in your library after uploading.

If the AI gives an answer **without a citation**, it might be making a guess based on general knowledge rather than your rulebooks. Always double-check rules that don't have a source citation.

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

---

*Chronacle keeps your game running smoothly — so you can focus on what matters: telling great stories with your players.*
