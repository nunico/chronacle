---
translationKey: providers.local
locale: en
slug: ai-providers/local
title: Use a local answer model
summary: Connect Chronacle to an Ollama model running on your computer while keeping search indexing separate.
section: ai-providers
order: 3
headings:
  - id: connect-ollama
    text: Connect Ollama
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: local-model-caveats
    text: Local model caveats
    level: 2
---

<h2 id="connect-ollama">Connect Ollama</h2>

Run a model in Ollama, then point Chronacle at its local address and exact model name.

1. Install Ollama using its current instructions and download a chat model it supports.
2. Make sure Ollama is running and note the model's exact identifier.
3. In Chronacle, open **Settings** and choose **Ollama (local)** under **Provider**.
4. Enter the model identifier under **Model**.
5. Leave **Base URL** at `http://localhost:11434` unless your Ollama service uses another address.
6. Choose **Save & connect** and look for **Connected: ollama**.
7. Configure **Embedding provider** separately; Ollama supplies answers, not Chronacle's indexing mode.

<h2 id="expected-result">Expected result</h2>

**Connection status** names Ollama and the selected model, and Oracle questions use that running local service.

<h2 id="example">Example</h2>

With a model already available in Ollama, connect it and ask the **Winter Ledger** campaign:

> Which promise did Warden Eska make at the frozen gate?

Chronacle retrieves the relevant passage and asks Ollama to formulate the answer, which can include `[Source: "The Winter Ledger.pdf", p.54]`.

<h2 id="local-model-caveats">Local model caveats</h2>

- Model download size, memory use, speed, and answer quality vary. Check the model's requirements against your computer.
- Chronacle does not download or start the Ollama chat model for you; Ollama must be ready when you connect.
- `http://localhost:11434` is the default local address shown by Chronacle, not a guarantee that your Ollama setup uses it.
- The local Nomic and E5 options under **Embedding provider** build the search index. They are separate from the Ollama answer model.
