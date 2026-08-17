<script lang="ts">
  interface Props {
    size?: number;
    glow?: boolean;
    label?: string;
  }

  let { size = 34, glow = true, label }: Props = $props();

  const uid = $props.id();
  let height = $derived(Math.round(size * 0.72));
  let filter = $derived(glow ? 'drop-shadow(0 0 10px rgba(123, 92, 255, 0.55))' : 'none');
</script>

<svg
  width={size}
  {height}
  viewBox="0 0 84 60"
  style:filter
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  {#if label}<title>{label}</title>{/if}
  <defs>
    <radialGradient id={`${uid}-iris`} cx="50%" cy="46%" r="60%">
      <stop offset="0%" stop-color="#EAF0FF" />
      <stop offset="34%" stop-color="#C8D6FF" />
      <stop offset="62%" stop-color="#7B5CFF" />
      <stop offset="100%" stop-color="#2A3FE0" />
    </radialGradient>
    <linearGradient id={`${uid}-lid`} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#8AA0FF" />
      <stop offset="100%" stop-color="#5B78FF" />
    </linearGradient>
  </defs>
  <path
    d="M4 30 C 22 6, 62 6, 80 30 C 62 54, 22 54, 4 30 Z"
    fill="#070912"
    stroke={`url(#${uid}-lid)`}
    stroke-width="2.4"
  />
  <circle cx="42" cy="30" r="16" fill={`url(#${uid}-iris)`} />
  <circle cx="42" cy="30" r="6.5" fill="#05060F" />
  <circle cx="47" cy="24" r="2.6" fill="#EAF0FF" />
</svg>
