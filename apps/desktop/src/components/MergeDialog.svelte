<script lang="ts">
  import {
    getEntity,
    getEntityRelations,
    mergeEntities,
    type EntityKind,
    type EntityError,
    type FieldChoice,
    type GraphNode,
  } from '../lib/commands';
  import { modalBehavior } from '../lib/actions/modal';
  import { i18n } from '../lib/locale.svelte';
  import Button from './ui/Button.svelte';

  interface Props {
    idA: string;
    kindA: EntityKind;
    idB: string;
    kindB: EntityKind;
    onclose?: () => void;
    onmerged?: () => void;
  }

  let { idA, kindA, idB, kindB, onclose, onmerged }: Props = $props();

  let nodeA = $state<GraphNode | null>(null);
  let nodeB = $state<GraphNode | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let survivor = $state<'a' | 'b'>('a');
  let summaryChoice = $state<FieldChoice>('keepSurvivor');
  let notesChoice = $state<FieldChoice>('keepSurvivor');
  let relationCount = $state<number | null>(null);
  let busy = $state(false);
  let mergeError = $state<string | null>(null);

  // The record to merge is always the id/kind PASSED IN (from the caller's
  // finding payload) — never the id/kind read back off the fetched node.
  // Those normally agree, but keeping the merge call keyed on the props (not
  // on fetch results used only for display) means a stale or coalesced
  // display fetch can never point the merge at the wrong record.
  let survivorRef = $derived(
    survivor === 'a' ? { id: idA, kind: kindA } : { id: idB, kind: kindB },
  );
  let loserRef = $derived(survivor === 'a' ? { id: idB, kind: kindB } : { id: idA, kind: kindA });

  let survivorNode = $derived(survivor === 'a' ? nodeA : nodeB);
  let loserNode = $derived(survivor === 'a' ? nodeB : nodeA);

  let consequence = $derived.by(() => {
    if (!loserNode) return '';
    const rc = relationCount;
    const relPart =
      rc === null
        ? '…'
        : i18n.t(rc === 1 ? 'entityUi.relationshipOne' : 'entityUi.relationshipMany', {
            count: rc,
          });
    const altCount = loserNode.aliases.length + 1;
    return i18n.t('entityUi.mergeConsequence', {
      relationships: relPart,
      names: i18n.t(altCount === 1 ? 'entityUi.alternateNameOne' : 'entityUi.alternateNameMany', {
        count: altCount,
      }),
    });
  });

  $effect(() => {
    let cancelled = false;
    loading = true;
    loadError = null;
    Promise.all([getEntity(idA, kindA), getEntity(idB, kindB)]).then(
      ([a, b]) => {
        if (cancelled) return;
        nodeA = a;
        nodeB = b;
        loading = false;
      },
      (e) => {
        if (cancelled) return;
        loadError = (e as EntityError).message ?? i18n.t('entityUi.failedLoadEntities');
        loading = false;
      },
    );
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const loser = loserRef;
    let cancelled = false;
    relationCount = null;
    getEntityRelations(loser.id, loser.kind).then(
      (rels) => {
        if (!cancelled) relationCount = rels.length;
      },
      () => {
        if (!cancelled) relationCount = 0;
      },
    );
    return () => {
      cancelled = true;
    };
  });

  async function handleMerge() {
    busy = true;
    mergeError = null;
    try {
      await mergeEntities(
        `${survivorRef.kind}:${survivorRef.id}`,
        `${loserRef.kind}:${loserRef.id}`,
        {
          summary: summaryChoice,
          notes: notesChoice,
        },
      );
      onmerged?.();
    } catch (e) {
      mergeError = (e as EntityError).message ?? i18n.t('entityUi.failedMerge');
    } finally {
      busy = false;
    }
  }
</script>

<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label={i18n.t('entityUi.mergeEntities')}
  use:modalBehavior={{ onClose: () => onclose?.() }}
>
  <div class="dialog">
    <h2 class="heading">{i18n.t('entityUi.mergeEntities')}</h2>

    {#if loading}
      <p class="muted">{i18n.t('entityUi.loading')}</p>
    {:else if loadError}
      <p class="error" role="alert">{loadError}</p>
    {:else if nodeA && nodeB}
      <div class="side-by-side">
        <label class="side" class:chosen={survivor === 'a'}>
          <input type="radio" name="merge-survivor" value="a" bind:group={survivor} />
          <span class="side-name">{nodeA.name}</span>
          <p class="side-summary">{nodeA.summary ?? i18n.t('entityUi.noSummary')}</p>
        </label>
        <label class="side" class:chosen={survivor === 'b'}>
          <input type="radio" name="merge-survivor" value="b" bind:group={survivor} />
          <span class="side-name">{nodeB.name}</span>
          <p class="side-summary">{nodeB.summary ?? i18n.t('entityUi.noSummary')}</p>
        </label>
      </div>
      <p class="hint">{i18n.t('entityUi.mergeHint')}</p>

      <div class="field-choice">
        <span class="field-choice-label">{i18n.t('entityUi.summary')}</span>
        <select bind:value={summaryChoice}>
          <option value="keepSurvivor"
            >{i18n.t('entityUi.keepEntity', { name: survivorNode?.name ?? '' })}</option
          >
          <option value="keepLoser"
            >{i18n.t('entityUi.keepEntity', { name: loserNode?.name ?? '' })}</option
          >
          <option value="keepBoth">{i18n.t('entityUi.keepBoth')}</option>
        </select>
      </div>
      <div class="field-choice">
        <span class="field-choice-label">{i18n.t('entityUi.notes')}</span>
        <select bind:value={notesChoice}>
          <option value="keepSurvivor"
            >{i18n.t('entityUi.keepEntity', { name: survivorNode?.name ?? '' })}</option
          >
          <option value="keepLoser"
            >{i18n.t('entityUi.keepEntity', { name: loserNode?.name ?? '' })}</option
          >
          <option value="keepBoth">{i18n.t('entityUi.keepBoth')}</option>
        </select>
      </div>

      <p class="consequence">{consequence}</p>

      {#if mergeError}
        <p class="error" role="alert">{mergeError}</p>
      {/if}

      <div class="actions">
        <Button loading={busy} loadingText={i18n.t('entityUi.merging')} onclick={handleMerge}
          >{i18n.t('entityUi.merge')}</Button
        >
        <Button variant="ghost" onclick={() => onclose?.()}>{i18n.t('common.cancel')}</Button>
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .dialog {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 20px;
    max-width: 520px;
    width: 90%;
  }
  .heading {
    margin: 0 0 12px;
    font-family: var(--font-display);
    color: var(--fg-1);
  }
  .muted {
    color: var(--fg-3);
    font-size: 0.85rem;
  }
  .side-by-side {
    display: flex;
    gap: 10px;
    margin-bottom: 6px;
  }
  .side {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px;
    cursor: pointer;
  }
  .side.chosen {
    border-color: var(--violet-400);
  }
  .side-name {
    font-weight: 600;
    color: var(--fg-1);
  }
  .side-summary {
    margin: 0;
    font-size: 0.8rem;
    color: var(--fg-3);
  }
  .hint {
    margin: 0 0 10px;
    font-size: 0.78rem;
    color: var(--fg-4, var(--fg-3));
  }
  .field-choice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 8px;
  }
  .field-choice-label {
    font-size: 0.85rem;
    color: var(--fg-3);
  }
  .field-choice select {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--fg-1);
    padding: 4px 8px;
    font-size: 0.85rem;
  }
  .consequence {
    font-size: 0.82rem;
    color: var(--fg-2);
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px 10px;
    margin: 10px 0;
  }
  .error {
    color: var(--danger);
    font-size: 0.82rem;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
</style>
