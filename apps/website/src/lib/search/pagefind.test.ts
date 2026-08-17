import { describe, expect, it, vi } from 'vitest';
import { createPagefindSearch } from './pagefind';

describe('Pagefind search adapter', () => {
  it('initializes Pagefind and maps no more than 20 results into the narrow contract', async () => {
    const init = vi.fn().mockResolvedValue(undefined);
    const data = Array.from({ length: 22 }, (_, index) =>
      vi.fn().mockResolvedValue(
        index === 1
          ? { url: 42, meta: { title: null, section: false }, excerpt: null }
          : index === 2
            ? { url: '/en/manual/missing-meta' }
            : {
                url: `/en/manual/article-${index}`,
                meta: { title: `Article ${index}`, section: 'Testing' },
                excerpt: `Excerpt ${index}`,
              },
      ),
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
    expect(mapped[1]).toEqual({ url: '', title: '', section: '', excerptHtml: '' });
    expect(mapped[2]).toEqual({
      url: '/en/manual/missing-meta',
      title: '',
      section: '',
      excerptHtml: '',
    });
    expect(data.slice(0, 20).every((loadData) => loadData.mock.calls.length === 1)).toBe(true);
    expect(data.slice(20).every((loadData) => loadData.mock.calls.length === 0)).toBe(true);
  });
});
