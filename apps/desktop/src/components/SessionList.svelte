<script lang="ts">
  import type { Session } from '../lib/commands';
  import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
  import SessionRow from './SessionRow.svelte';

  interface Props {
    campaignId: string;
    sessions: Session[];
    entityMap: Map<string, { id: string; kind: string }>;
    onUpdate: (session: Session) => void;
    onDelete: (id: string) => void;
    draftCoordinator?: DraftCoordinator;
  }

  let {
    campaignId,
    sessions,
    entityMap,
    onUpdate,
    onDelete,
    draftCoordinator = new DraftCoordinator(),
  }: Props = $props();
</script>

<div class="session-list">
  {#each sessions as session (`${campaignId}:${session.id}`)}
    <SessionRow {campaignId} {session} {entityMap} {onUpdate} {onDelete} {draftCoordinator} />
  {/each}
</div>

<style>
  .session-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
