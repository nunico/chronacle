import { describe, expect, it, vi } from 'vitest';
import { createPagefindSearch } from './pagefind';

describe('Pagefind search adapter', () => {
  it('initializes Pagefind and maps no more than 20 results into the narrow contract', async () => {
    const init = vi.fn().mockResolvedValue(undefined);
    const data = Array.from({ length: 22 }, (_, index) =>
      vi.fn().mockResolvedValue({
        url: `/en/manual/article-${index}`,
        meta: { title: `Article ${index}`, section: 'Testing' },
        excerpt: `Excerpt ${index}`,
      }),
    );
    const search = vi.fn().mockResolvedValue({
      results: data.map((loadData) => ({ data: loadData })),
    });
    const adapter = createPagefindSearch(async () => ({ init, search }));

    await adapter.init();
    const mapped = await adapter.search('fixture');

    expect(init).toHaveBeenCalledOnce();
    expect(search).toHaveBeenCalledWith('fixture');
    expect(mapped).toHaveLength(20);
    expect(mapped[0]).toEqual({
      url: '/en/manual/article-0',
      title: 'Article 0',
      section: 'Testing',
      excerptHtml: 'Excerpt 0',
    });
    expect(data.slice(0, 20).every((loadData) => loadData.mock.calls.length === 1)).toBe(true);
    expect(data.slice(20).every((loadData) => loadData.mock.calls.length === 0)).toBe(true);
  });

  it('keeps valid hydrated results when another result fails and maps missing metadata safely', async () => {
    const search = vi.fn().mockResolvedValue({
      results: [
        { data: vi.fn().mockRejectedValue(new Error('fragment unavailable')) },
        { data: vi.fn().mockResolvedValue({ url: '/en/manual/missing-meta#details' }) },
        {
          data: vi.fn().mockResolvedValue({
            url: '/de/handbuch/fehlerbehebung/haeufige-probleme/',
            meta: { title: null, section: false },
            excerpt: null,
          }),
        },
      ],
    });
    const adapter = createPagefindSearch(async () => ({
      init: vi.fn().mockResolvedValue(undefined),
      search,
    }));

    await expect(adapter.search('fixture')).resolves.toEqual([
      {
        url: '/en/manual/missing-meta#details',
        title: '',
        section: '',
        excerptHtml: '',
      },
      {
        url: '/de/handbuch/fehlerbehebung/haeufige-probleme/',
        title: '',
        section: '',
        excerptHtml: '',
      },
    ]);
  });

  it('discards results without a canonical internal manual URL', async () => {
    const invalidUrls: unknown[] = [
      '',
      undefined,
      42,
      'https://example.com/en/manual',
      'javascript:alert(1)',
      '/en/about',
      '/fr/manual',
      '//example.com/en/manual',
      '/en/manual/../admin',
    ];
    const search = vi.fn().mockResolvedValue({
      results: invalidUrls.map((url) => ({
        data: vi.fn().mockResolvedValue({ url, meta: {}, excerpt: '' }),
      })),
    });
    const adapter = createPagefindSearch(async () => ({
      init: vi.fn().mockResolvedValue(undefined),
      search,
    }));

    await expect(adapter.search('fixture')).resolves.toEqual([]);
  });
});
