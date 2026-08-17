declare module '/pagefind/pagefind.js' {
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

  export function init(): Promise<void>;
  export function search(query: string): Promise<PagefindResponse>;
}
