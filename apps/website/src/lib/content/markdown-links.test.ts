import { collectManualLinks } from './markdown-links.js';

describe('Markdown link collection', () => {
  it('collects inline and referenced Markdown links into module metadata', () => {
    const frontmatter: Record<string, unknown> = {};
    const tree = {
      type: 'root',
      children: [
        { type: 'link', url: '/en/manual/inline', children: [] },
        { type: 'linkReference', identifier: 'Guide', children: [] },
        { type: 'definition', identifier: 'guide', url: './referenced' },
      ],
    };

    collectManualLinks()(tree, { data: { fm: frontmatter } });

    expect(frontmatter.manualLinks).toEqual(['/en/manual/inline', './referenced']);
  });
});
