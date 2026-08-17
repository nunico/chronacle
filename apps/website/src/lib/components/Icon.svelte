<script lang="ts">
  import type { Icon as LucideIcon } from 'lucide-svelte';

  interface Props {
    icon: typeof LucideIcon;
    size?: number;
    strokeWidth?: number;
    label?: string;
    class?: string;
  }

  let {
    icon: Glyph,
    size = 18,
    strokeWidth = 1.75,
    label,
    class: className = '',
  }: Props = $props();
</script>

<span
  class={['icon', className]}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
  style:--icon-size={`${size}px`}
>
  <Glyph {size} {strokeWidth} aria-hidden="true" />
</span>

<style>
  .icon {
    display: inline-flex;
    width: var(--icon-size);
    height: var(--icon-size);
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    line-height: 0;
  }
</style>
