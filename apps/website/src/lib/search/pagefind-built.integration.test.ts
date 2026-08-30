import { createServer, type Server } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

interface BuiltResultData {
  url: string;
}

interface BuiltResult {
  data(): Promise<BuiltResultData>;
}

interface BuiltSearchResponse {
  results: BuiltResult[];
}

interface BuiltInstance {
  init(): Promise<void>;
  search(query: string): Promise<BuiltSearchResponse>;
  destroy(): Promise<void>;
}

interface BuiltPagefind {
  createInstance(options: { basePath: string; noWorker: boolean }): BuiltInstance;
}

const buildRoot = resolve(process.cwd(), 'build');
const pagefindModule = pathToFileURL(resolve(buildRoot, 'pagefind/pagefind.js'));
let server: Server;
let origin = '';

beforeAll(async () => {
  server = createServer((request, response) => {
    const requestUrl = new URL(request.url ?? '/', 'http://localhost');
    const filePath = resolve(buildRoot, requestUrl.pathname.slice(1));
    if (!filePath.startsWith(`${buildRoot}${sep}`)) {
      response.writeHead(404).end();
      return;
    }
    void readFile(filePath)
      .then((body) => {
        response.setHeader(
          'content-type',
          requestUrl.pathname.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream',
        );
        response.end(body);
      })
      .catch(() => response.writeHead(404).end());
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('Pagefind test server did not expose a TCP port');
  }
  origin = `http://127.0.0.1:${address.port}`;
});

afterAll(async () => {
  await new Promise<void>((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
});

async function searchFor(locale: 'en' | 'de', query: string): Promise<string[]> {
  document.documentElement.lang = locale;
  const pagefind = (await import(
    /* @vite-ignore */ pagefindModule.href
  )) as unknown as BuiltPagefind;
  const instance = pagefind.createInstance({
    basePath: `${origin}/pagefind/`,
    noWorker: true,
  });
  await instance.init();
  const response = await instance.search(query);
  const data = await Promise.all(response.results.map((result) => result.data()));
  await instance.destroy();
  return data.map((result) => {
    const url = new URL(result.url, origin);
    return `${url.pathname}${url.hash}`;
  });
}

describe('built Pagefind index', () => {
  it('keeps English and German manual results in their current-language indexes', async () => {
    const englishUrls = await searchFor('en', 'important steps');
    const germanUrls = await searchFor('de', 'Abschnittsindex');

    expect(englishUrls.length).toBeGreaterThan(0);
    expect(englishUrls.every((url) => url.startsWith('/en/manual/'))).toBe(true);
    expect(germanUrls.length).toBeGreaterThan(0);
    expect(germanUrls.every((url) => url.startsWith('/de/handbuch/'))).toBe(true);
  });
});
