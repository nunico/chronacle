use serde_json::Value;
use tokio::sync::mpsc;

use super::LlmError;

/// Parse an OpenAI-style SSE stream and push tokens through the channel.
///
/// OpenAI SSE format:
/// ```text
/// data: {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"Hello"},"index":0}]}
///
/// data: [DONE]
/// ```
pub(super) async fn openai_parse_sse(
    mut response: reqwest::Response,
    tx: mpsc::Sender<Result<String, LlmError>>,
) {
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);

                // Process all complete events in the buffer.
                // Each SSE event is: `data: <json>\n\n`, possibly with `data: [DONE]\n\n`.
                let mut consumed = 0usize;
                while consumed < buf.len() {
                    // Find the next \n\n boundary
                    let remaining = &buf[consumed..];
                    if let Some(dd) = remaining.windows(2).position(|w| w == b"\n\n") {
                        let event_end = consumed + dd + 2;
                        let event_slice = &buf[consumed..event_end];
                        let raw = String::from_utf8_lossy(event_slice);

                        for line in raw.lines() {
                            let line = line.trim();
                            if let Some(data) = line.strip_prefix("data: ") {
                                let data = data.trim();
                                if data == "[DONE]" {
                                    // Signal the end (no more tokens).
                                    return;
                                }

                                match serde_json::from_str::<Value>(data) {
                                    Ok(json) => {
                                        // Extract text from choices[0].delta.content
                                        if let Some(content) = json
                                            .pointer("/choices/0/delta/content")
                                            .and_then(|v| v.as_str())
                                        {
                                            if !content.is_empty()
                                                && tx.send(Ok(content.to_string())).await.is_err()
                                            {
                                                return; // receiver dropped
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("SSE parse warning — invalid JSON: {e}");
                                    }
                                }
                            }
                        }

                        consumed = event_end;
                    } else {
                        break; // wait for more data
                    }
                }

                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            Ok(None) => return, // stream ended normally
            Err(e) => {
                let _ = tx.send(Err(LlmError::Connection(e.to_string()))).await;
                return;
            }
        }
    }
}

/// Parse an Anthropic-style SSE stream and push tokens through the channel.
///
/// Anthropic SSE format:
/// ```text
/// event: content_block_delta
/// data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}
///
/// event: message_stop
/// data: {"type":"message_stop"}
/// ```
pub(super) async fn anthropic_parse_sse(
    mut response: reqwest::Response,
    tx: mpsc::Sender<Result<String, LlmError>>,
) {
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);

                let mut consumed = 0usize;
                while consumed < buf.len() {
                    let remaining = &buf[consumed..];
                    if let Some(dd) = remaining.windows(2).position(|w| w == b"\n\n") {
                        let event_end = consumed + dd + 2;
                        let event_slice = &buf[consumed..event_end];
                        let raw = String::from_utf8_lossy(event_slice);

                        // Track the current event type from `event:` lines
                        let mut event_type: Option<&str> = None;
                        let mut data_json: Option<&str> = None;

                        for line in raw.lines() {
                            let line = line.trim();
                            if let Some(ev) = line.strip_prefix("event: ") {
                                event_type = Some(ev.trim());
                            } else if let Some(d) = line.strip_prefix("data: ") {
                                data_json = Some(d.trim());
                            }
                        }

                        // Only extract text from content_block_delta events
                        if event_type == Some("content_block_delta") {
                            if let Some(json_str) = data_json {
                                if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                                    if let Some(text) =
                                        json.pointer("/delta/text").and_then(|v| v.as_str())
                                    {
                                        if !text.is_empty()
                                            && tx.send(Ok(text.to_string())).await.is_err()
                                        {
                                            return;
                                        }
                                    }
                                }
                            }
                        }

                        consumed = event_end;
                    } else {
                        break;
                    }
                }

                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            Ok(None) => return,
            Err(e) => {
                let _ = tx.send(Err(LlmError::Connection(e.to_string()))).await;
                return;
            }
        }
    }
}

/// Parse an Ollama NDJSON stream and push tokens through the channel.
///
/// Ollama NDJSON format (one JSON object per line, `done: true` signals end):
/// ```json
/// {"model":"...","message":{"role":"assistant","content":"Hello"},"done":false}
/// {"model":"...","message":{"role":"assistant","content":""},"done_reason":"stop","done":true}
/// ```
pub(super) async fn ollama_parse_ndjson(
    mut response: reqwest::Response,
    tx: mpsc::Sender<Result<String, LlmError>>,
) {
    let mut buf: Vec<u8> = Vec::new();

    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk);

                // Process complete newline-delimited JSON objects
                let mut consumed = 0usize;
                while consumed < buf.len() {
                    let remaining = &buf[consumed..];
                    if let Some(nl) = remaining.iter().position(|&b| b == b'\n') {
                        let line_end = consumed + nl + 1; // include the newline
                        let line_slice = &buf[consumed..(consumed + nl)];
                        let line = String::from_utf8_lossy(line_slice).trim().to_string();

                        if !line.is_empty() {
                            match serde_json::from_str::<Value>(&line) {
                                Ok(json) => {
                                    // Extract message.content
                                    if let Some(content) =
                                        json.pointer("/message/content").and_then(|v| v.as_str())
                                    {
                                        if !content.is_empty()
                                            && tx.send(Ok(content.to_string())).await.is_err()
                                        {
                                            return;
                                        }
                                    }

                                    // Check if done
                                    if json.get("done").and_then(|v| v.as_bool()) == Some(true) {
                                        return;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Ollama NDJSON parse warning: {e}");
                                }
                            }
                        }

                        consumed = line_end;
                    } else {
                        break; // wait for more data
                    }
                }

                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            Ok(None) => return,
            Err(e) => {
                let _ = tx.send(Err(LlmError::Connection(e.to_string()))).await;
                return;
            }
        }
    }
}
