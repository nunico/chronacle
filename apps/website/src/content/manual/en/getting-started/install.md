---
translationKey: getting-started.install
locale: en
slug: getting-started/install
title: Install Chronacle
summary: Download the current release for your operating system and complete the first launch.
section: getting-started
order: 2
headings:
  - id: install-the-current-release
    text: Install the current release
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: first-launch-notes
    text: First-launch notes
    level: 2
---

<h2 id="install-the-current-release">Install the current release</h2>

Get Chronacle from its current GitHub release and choose the download that matches your operating system.

1. Open the [current Chronacle release](https://github.com/nunico/chronacle/releases/latest).
2. In the release assets, choose the build for your macOS, Windows, or Linux computer.
3. Open the downloaded package and follow the prompts shown by your operating system.
4. Launch Chronacle.
5. If Chronacle shows **AI model required**, choose **Start download**. Keep the app open until it reports **Model ready!**

<h2 id="expected-result">Expected result</h2>

Chronacle opens to its main window, or briefly shows the local search-model download before opening the main window.

<h2 id="example">Example</h2>

After installing on the computer you use for **Lanterns at Dusk**, open Chronacle, finish the model download if prompted, connect your provider, and import `Harbor of Glass.pdf`. You are then ready to ask:

> Which bell signals that the glass tide is turning?

The answer should include a source badge such as `[Source: "Harbor of Glass.pdf", p.18]` when that passage is found.

<h2 id="first-launch-notes">First-launch notes</h2>

- Release assets can differ by operating system and processor. Use the labels on the current release rather than relying on an old filename.
- The local indexing model is not the model that writes answers. Continue with [choosing an AI provider](/en/manual/ai-providers/choose).
- If a download fails, use **Retry**. The displayed error is the useful starting point; do not guess at the cause.
