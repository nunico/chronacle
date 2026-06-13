// Deterministic OpenAI-compatible LLM stub for the enrichment E2E.
//
// The real `OpenAIProvider` posts `{ ..., stream: true }` to
// `<base_url>/chat/completions` and parses an SSE stream, reading
// `choices[0].delta.content` (see src-tauri/src/providers/llm_provider.rs).
// This server replies with a single SSE delta carrying the full canned JSON,
// then `[DONE]` — enough for `extraction_service::llm_complete` to reassemble.
//
// It branches on the prompt: the seed/extraction prompt asks for a `"relations"`
// array, the second-pass profile prompt does not. That lets one endpoint serve
// both passes deterministically.
import { createServer } from 'node:http';

// What the model "returns" for the first (seed) pass: the seed NPC plus one
// related faction whose summary is deliberately RELATIONAL (describes its tie
// to the seed) — this is the summary enrichment must overwrite.
export const SEED_JSON = JSON.stringify({
  entities: [
    {
      name: 'Commander Varn',
      kind: 'npc',
      summary: 'A ruthless commander who rose from the slums.',
      notes: null,
      relations: [
        {
          name: 'The Iron Fist',
          kind: 'faction',
          rel_type: 'commands',
          summary: 'The militia that Commander Varn commands.',
          notes: null,
        },
      ],
    },
  ],
});

// What the model "returns" for the second (profile) pass: an ENTITY-CENTRIC
// summary describing the faction itself, with the relational detail moved to
// notes. The assertion checks the faction ends up with exactly this summary.
export const PROFILE_SUMMARY =
  'A militant guild controlling the eastern docks of Varrowmoor.';
export const PROFILE_JSON = JSON.stringify({
  summary: PROFILE_SUMMARY,
  notes: 'Led by [[Commander Varn]].',
});

/** The relational summary that must NOT survive enrichment. */
export const RELATIONAL_SUMMARY = 'The militia that Commander Varn commands.';

function sse(res, content) {
  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    Connection: 'keep-alive',
  });
  const chunk = { choices: [{ delta: { content }, index: 0 }] };
  res.write(`data: ${JSON.stringify(chunk)}\n\n`);
  res.write('data: [DONE]\n\n');
  res.end();
}

/**
 * Start the stub. Returns `{ url, calls, close }` where `url` is the OpenAI
 * base URL to feed into the `llm_base_url` setting and `calls` records each
 * pass seen ('seed' | 'profile') for later inspection.
 */
export function startStubLlm() {
  const calls = [];
  const server = createServer((req, res) => {
    if (req.method !== 'POST' || !req.url.endsWith('/chat/completions')) {
      res.writeHead(404).end();
      return;
    }
    let body = '';
    req.on('data', (d) => (body += d));
    req.on('end', () => {
      const isSeed = body.includes('\\"relations\\"') || body.includes('"relations"');
      if (isSeed) {
        calls.push('seed');
        sse(res, SEED_JSON);
      } else {
        calls.push('profile');
        sse(res, PROFILE_JSON);
      }
    });
  });

  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}/v1`,
        calls,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
