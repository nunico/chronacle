<script lang="ts">
  import type { Session } from '../lib/commands';
  import SessionRow from './SessionRow.svelte';

  interface Props {
    sessions: Session[];
    entityMap: Map<string, { id: string; kind: string }>;
    onUpdate: (session: Session) => void;
    onDelete: (id: string) => void;
  }

  const { sessions, entityMap, onUpdate, onDelete }: Props = $props();
</script>

<div class="session-list">
  {#each sessions as session (session.id)}
    <SessionRow {session} {entityMap} {onUpdate} {onDelete} />
  {/each}
</div>

<style>
  .session-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
