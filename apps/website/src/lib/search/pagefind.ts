import type { ManualSearch, SearchResult } from './types';

interface PagefindData {
  excerpt?: unknown;
  meta?: unknown;
  url?: unknown;
}

interface PagefindResult {
  data(): Promise<PagefindData>;
}

interface PagefindResponse {
  results?: unknown;
}

interface PagefindModule {
  init(): Promise<void>;
  search(query: string): Promise<PagefindResponse>;
}

const pagefindPath = '/pagefind/pagefind.js';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isPagefindModule(value: unknown): value is PagefindModule {
  return isRecord(value) && typeof value.init === 'function' && typeof value.search === 'function';
}

function isPagefindResult(value: unknown): value is PagefindResult {
  return isRecord(value) && typeof value.data === 'function';
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function mapData(data: PagefindData): SearchResult {
  const meta = isRecord(data.meta) ? data.meta : {};
  return {
    url: stringValue(data.url),
    title: stringValue(meta.title),
    section: stringValue(meta.section),
    excerptHtml: stringValue(data.excerpt),
  };
}

export function createPagefindSearch(loader: () => Promise<unknown>): ManualSearch {
  let modulePromise: Promise<PagefindModule> | undefined;

  function loadPagefind(): Promise<PagefindModule> {
    modulePromise ??= loader().then((pagefind) => {
      if (!isPagefindModule(pagefind)) {
        throw new Error('Invalid Pagefind module');
      }
      return pagefind;
    });
    return modulePromise;
  }

  return {
    async init(): Promise<void> {
      const pagefind = await loadPagefind();
      await pagefind.init();
    },

    async search(query: string): Promise<SearchResult[]> {
      const pagefind = await loadPagefind();
      const response = await pagefind.search(query);
      const rawResults = Array.isArray(response.results) ? response.results : [];
      const data = await Promise.all(
        rawResults
          .filter(isPagefindResult)
          .slice(0, 20)
          .map((result) => result.data()),
      );
      return data.map(mapData);
    },
  };
}

export const pagefindSearch = createPagefindSearch(() => import(/* @vite-ignore */ pagefindPath));
