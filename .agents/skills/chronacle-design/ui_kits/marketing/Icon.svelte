<script>
  /* Icon — Lucide line-icon wrapper. Renders imperatively in an effect. */
  let { name, size = 18, color = "", strokeWidth = 1.75, style = "", className = "" } = $props();
  let el = $state();

  $effect(() => {
    name; size; strokeWidth; // track
    if (!el || !window.lucide) return;
    el.innerHTML = "";
    const i = document.createElement("i");
    i.setAttribute("data-lucide", name);
    el.appendChild(i);
    try {
      window.lucide.createIcons({ attrs: { width: size, height: size, "stroke-width": strokeWidth } });
    } catch (e) { /* ignore */ }
  });
</script>

<span
  bind:this={el}
  class={className}
  style="display:inline-flex;align-items:center;justify-content:center;width:{size}px;height:{size}px;line-height:0;{color ? `color:${color};` : ''}{style}"
></span>
