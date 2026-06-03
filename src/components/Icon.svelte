<script lang="ts">
  import { icons, createElement, type IconNode } from 'lucide';

  let {
    name,
    size = 18,
    strokeWidth = 1.75,
    color = '',
    className = '',
  }: {
    name: string;
    size?: number;
    strokeWidth?: number;
    color?: string;
    className?: string;
  } = $props();

  let el = $state<HTMLSpanElement | undefined>(undefined);

  // Lucide names are kebab-case ("book-open"); the icons map is keyed PascalCase ("BookOpen").
  function toPascal(kebab: string): string {
    return kebab
      .split('-')
      .map((p) => (p ? p[0].toUpperCase() + p.slice(1) : ''))
      .join('');
  }

  $effect(() => {
    if (!el) return;
    const node = (icons as Record<string, IconNode>)[toPascal(name)];
    el.innerHTML = '';
    if (!node) return;
    const svg = createElement(node);
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('stroke-width', String(strokeWidth));
    el.appendChild(svg);
  });
</script>

<span
  bind:this={el}
  class={className}
  style="display:inline-flex;align-items:center;justify-content:center;width:{size}px;height:{size}px;line-height:0;{color
    ? `color:${color};`
    : ''}"
></span>
