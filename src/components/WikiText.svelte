<script lang="ts">
  const WIKILINK_RE = /\[\[([^\]]+)\]\]/g;

  type Segment =
    | { kind: 'text'; content: string }
    | { kind: 'entity'; name: string; id: string; entityKind: string }
    | { kind: 'unmatched'; name: string };

  interface Props {
    text: string;
    entities: Map<string, { id: string; kind: string }>;
    onEntityClick?: (id: string, kind: string) => void;
  }

  const { text, entities, onEntityClick }: Props = $props();

  const segments = $derived.by((): Segment[] => {
    const result: Segment[] = [];
    let last = 0;
    let m: RegExpExecArray | null;
    WIKILINK_RE.lastIndex = 0;
    while ((m = WIKILINK_RE.exec(text)) !== null) {
      if (m.index > last) {
        result.push({ kind: 'text', content: text.slice(last, m.index) });
      }
      const name = m[1];
      const key = [...entities.keys()].find(k => k.toLowerCase() === name.toLowerCase());
      if (key) {
        const e = entities.get(key)!;
        result.push({ kind: 'entity', name, id: e.id, entityKind: e.kind });
      } else {
        result.push({ kind: 'unmatched', name });
      }
      last = m.index + m[0].length;
    }
    if (last < text.length) {
      result.push({ kind: 'text', content: text.slice(last) });
    }
    return result;
  });
</script>

<span class="wiki-text">
  {#each segments as seg, i (i)}
    {#if seg.kind === 'text'}
      {seg.content}
    {:else if seg.kind === 'entity'}
      <button class="entity-badge" title={seg.entityKind} onclick={() => onEntityClick?.(seg.id, seg.entityKind)}>
        {seg.name}
      </button>
    {:else}
      [[{seg.name}]]
    {/if}
  {/each}
</span>

<style>
  .wiki-text {
    font-family: inherit;
    font-size: inherit;
    line-height: inherit;
  }
</style>
