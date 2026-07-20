<script lang="ts">
  import { normalizeWikiLinkKey } from '../lib/wikilinks';
  import { i18n } from '../lib/locale.svelte';

  const WIKILINK_RE = /\[\[([^\]]+)\]\]/g;

  type Segment =
    | { kind: 'text'; content: string }
    | { kind: 'entity'; name: string; id: string; entityKind: string }
    | { kind: 'unmatched'; name: string };

  interface Props {
    text: string;
    entities: Map<string, { id: string; kind: string }>;
    onEntityClick?: (id: string, kind: string) => void;
    onMissingLinkClick?: (name: string) => void;
  }

  const { text, entities, onEntityClick, onMissingLinkClick }: Props = $props();

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
      const lowerKey = name.toLowerCase();
      const entity =
        entities.get(name) ??
        entities.get(lowerKey) ??
        entities.get(normalizeWikiLinkKey(name)) ??
        entities.get([...entities.keys()].find((k) => k.toLowerCase() === lowerKey) ?? '');
      if (entity) {
        const e = entity as { id: string; kind: string };
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
  {#each segments as seg, i (`${seg.kind}-${seg.kind === 'text' ? seg.content : seg.name}-${i}`)}
    {#if seg.kind === 'text'}
      {seg.content}
    {:else if seg.kind === 'entity'}
      <button
        type="button"
        class="entity-badge"
        title={seg.entityKind}
        onclick={() => onEntityClick?.(seg.id, seg.entityKind)}
      >
        {seg.name}
      </button>
    {:else}
      {#if onMissingLinkClick}
        <button
          type="button"
          class="missing-link"
          aria-label={i18n.t('shell.createArticleAria', { name: seg.name })}
          onclick={() => onMissingLinkClick(seg.name)}
        >
          [[{seg.name}]]
        </button>
      {:else}
        [[{seg.name}]]
      {/if}
    {/if}
  {/each}
</span>

<style>
  .wiki-text {
    font-family: inherit;
    font-size: inherit;
    line-height: inherit;
  }

  .missing-link {
    border: 0;
    border-bottom: 1px dashed currentcolor;
    background: transparent;
    color: inherit;
    cursor: pointer;
    font: inherit;
    padding: 0;
  }
</style>
