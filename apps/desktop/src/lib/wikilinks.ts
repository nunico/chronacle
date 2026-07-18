import type { GraphNode } from './commands';

export interface WikiLinkTarget {
  id: string;
  kind: string;
}

type NodeLike = Pick<GraphNode, 'id' | 'kind' | 'name' | 'aliases'>;

export function normalizeWikiLinkKey(name: string): string {
  let key = name
    .trim()
    .toLowerCase()
    .replace(/['\u2019]s\b/g, '')
    .replace(/^the\s+/u, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim()
    .replace(/\s+/g, ' ');

  key = key
    .split(' ')
    .map(part => singularize(part))
    .join(' ');
  return key;
}

function singularize(part: string): string {
  if (part.endsWith('ss') || part.endsWith('us') || part.length <= 3) return part;
  if (part.endsWith('ies') && part.length > 4) return `${part.slice(0, -3)}y`;
  if (part.endsWith('s')) return part.slice(0, -1);
  return part;
}

export function buildWikiLinkEntityMap(nodes: NodeLike[]): Map<string, WikiLinkTarget> {
  const values = new Map<string, WikiLinkTarget>();
  const collisions = new Set<string>();

  function insert(key: string, target: WikiLinkTarget) {
    if (!key || collisions.has(key)) return;
    const existing = values.get(key);
    if (existing && (existing.id !== target.id || existing.kind !== target.kind)) {
      values.delete(key);
      collisions.add(key);
      return;
    }
    values.set(key, target);
  }

  for (const node of nodes) {
    const target = { id: node.id, kind: node.kind };
    for (const raw of [node.name, ...(node.aliases ?? [])]) {
      insert(raw.trim().toLowerCase(), target);
      insert(normalizeWikiLinkKey(raw), target);
    }
  }

  return values;
}
