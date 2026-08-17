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
