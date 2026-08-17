import type { Pathname } from '$app/types';

export interface SearchResult {
  url: string;
  title: string;
  section: string;
  excerptHtml: string;
}

export interface ManualSearch {
  init(): Promise<void>;
  search(query: string): Promise<SearchResult[]>;
}

const manualPathPattern =
  /^\/(?:en\/manual|de\/handbuch)(?:\/[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*)?\/?(?:#[^\s/?#]+)?$/;

export function asManualPathname(url: string): Pathname | undefined {
  return manualPathPattern.test(url) ? (url as Pathname) : undefined;
}
