/** Map a collection name (lowercased) to a Lucide icon name. Falls back to 'book-open'. */
export function collectionIcon(name: string): string {
  const n = name.toLowerCase();
  if (n.includes('rule')) return 'book-open';
  if (n.includes('lore') || n.includes('codex') || n.includes('realm')) return 'castle';
  if (n.includes('home') || n.includes('table')) return 'scroll-text';
  if (n.includes('best') || n.includes('monster') || n.includes('creature')) return 'paw-print';
  return 'book-open';
}
