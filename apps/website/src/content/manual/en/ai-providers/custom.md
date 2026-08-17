---
translationKey: providers.custom
locale: en
slug: ai-providers/custom
title: Add a custom provider
summary: Register an OpenAI-compatible or Anthropic-compatible endpoint and add the models you want Chronacle to offer.
section: ai-providers
order: 4
headings:
  - id: register-the-provider
    text: Register the provider
    level: 2
  - id: expected-result
    text: Expected result
    level: 2
  - id: example
    text: Example
    level: 2
  - id: compatibility-checks
    text: Compatibility checks
    level: 2
---

<h2 id="register-the-provider">Register the provider</h2>

Add the service address once, attach one or more model IDs, then select it as the active answer provider. Here, an endpoint is simply the web address Chronacle contacts.

1. In **Settings**, open **Custom providers** and choose **Add custom provider**.
2. Enter a recognizable **Provider name**.
3. Choose **OpenAI-compatible** or **Anthropic-compatible** under **API compatibility**.
4. Enter the provider's **Base URL** and an API key. Although the registration form labels the key **API key (optional)**, the current **Save & connect** flow requires a nonempty key for every custom provider.
5. Choose **Save provider**.
6. On the new provider card, choose **Add model**. Enter the exact **Model ID** used by the service and a clear display name, then choose **Add**.
7. Return to **LLM provider**, select **Custom: your name**, choose the model, and select **Save & connect**.

<h2 id="expected-result">Expected result</h2>

The custom provider appears in the provider list, its model appears in the model selector, and a successful connection is reflected under **Connection status**.

<h2 id="example">Example</h2>

Register a service as **Cinder Gateway**, choose the compatibility type documented by that service, enter `https://ai.cinder.example/v1`, and add its factual model ID `cinder-chat-small` with the display name **Cinder Chat Small**. After connecting, ask:

> What does Magistrate Oren hide beneath the copper dais?

With `Court of Cinders.pdf` in a subscribed collection, the reply can cite `[Source: "Court of Cinders.pdf", p.91]`.

<h2 id="compatibility-checks">Compatibility checks</h2>

- Compatibility refers to the request format the service accepts; it does not mean every model or feature behaves identically.
- Use the provider's documented base URL and model ID. Chronacle does not discover model IDs automatically.
- A service that accepts keyless requests cannot currently be connected through **Save & connect**, because Chronacle rejects an empty custom-provider key before testing the service.
- Custom answer-provider setup does not configure cloud indexing. Set that separately under **Embedding provider** if needed.
