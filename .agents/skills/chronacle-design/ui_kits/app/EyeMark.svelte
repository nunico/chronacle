<script>
  /* EyeMark — the scrying-eye avatar (almond lens + arcane iris + pupil + glint). */
  let { size = 34, glow = true } = $props();
  const uid = "e" + Math.random().toString(36).slice(2, 9);
  let h = $derived(Math.round(size * 0.72));
</script>

<svg width={size} height={h} viewBox="0 0 84 60" style="display:block;filter:{glow ? 'drop-shadow(0 0 10px rgba(123,92,255,0.55))' : 'none'}">
  <defs>
    <radialGradient id={"ir" + uid} cx="50%" cy="46%" r="60%">
      <stop offset="0%" stop-color="#EAF0FF" />
      <stop offset="34%" stop-color="#C8D6FF" />
      <stop offset="62%" stop-color="#7B5CFF" />
      <stop offset="100%" stop-color="#2A3FE0" />
    </radialGradient>
    <linearGradient id={"ld" + uid} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#8AA0FF" />
      <stop offset="100%" stop-color="#5B78FF" />
    </linearGradient>
  </defs>
  <path d="M4 30 C 22 6, 62 6, 80 30 C 62 54, 22 54, 4 30 Z" fill="#070912" stroke={"url(#ld" + uid + ")"} stroke-width="2.4" />
  <circle cx="42" cy="30" r="16" fill={"url(#ir" + uid + ")"} />
  <circle cx="42" cy="30" r="6.5" fill="#05060F" />
  <circle cx="47" cy="24" r="2.6" fill="#EAF0FF" />
</svg>
