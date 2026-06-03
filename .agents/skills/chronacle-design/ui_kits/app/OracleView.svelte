<script>
  /* Oracle view: the thread of questions & rulings + the ask composer. */
  import { CHRONACLE as D } from "./data.js";
  import Icon from "./Icon.svelte";
  import EyeMark from "./EyeMark.svelte";
  import RulingCard from "./RulingCard.svelte";

  let thread = $state([...D.seed]);
  let draft = $state("");
  let focus = $state(false);
  let thinking = $state(false);
  let scrollEl = $state();

  function pick(q) {
    const t = q.toLowerCase();
    if (t.includes("grappl")) return D.answers.grappl;
    if (t.includes("cover")) return D.answers.cover;
    if (t.includes("concord") || t.includes("lead")) return D.answers.concord;
    if (t.includes("ford") || t.includes("greywater")) return D.answers.ford;
    return D.answers._default;
  }

  function ask(q) {
    const question = (q || draft).trim();
    if (!question || thinking) return;
    thread = [...thread, { role: "user", text: question }];
    draft = "";
    thinking = true;
    setTimeout(() => {
      const a = pick(question);
      thread = [...thread, { role: "ruling", _fresh: true, ...a }];
      thinking = false;
    }, 1500);
  }

  $effect(() => {
    thread; thinking; // track
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  });
</script>

<div class="scroll" bind:this={scrollEl}>
  <div class="thread">
    {#each thread as m, i (i)}
      {#if m.role === "user"}
        <div class="msg user">
          <div class="who-av">GM</div>
          <div class="bubble">{m.text}</div>
        </div>
      {:else}
        <RulingCard data={m} defaultOpen={i === 1} />
      {/if}
    {/each}

    {#if thinking}
      <div class="msg">
        <div class="who-av eye-badge"><EyeMark size={28} /></div>
        <div class="thinking">
          <span class="tdot"></span><span class="tdot"></span><span class="tdot"></span>
          <span class="label">consulting your tomes…</span>
        </div>
      </div>
    {/if}

    {#if !thinking}
      <div class="suggest">
        {#each D.suggestions as s, i}
          <button class="sug" onclick={() => ask(s.text)}>
            <Icon className="ic" name={s.icon} size={15} />
            {s.text}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<div class="composer-wrap">
  <div class="composer{focus ? ' focus' : ''}">
    <Icon className="star" name="sparkles" size={20} />
    <input
      bind:value={draft}
      placeholder="Ask a rule, a name, a place…"
      onfocus={() => (focus = true)}
      onblur={() => (focus = false)}
      onkeydown={(e) => e.key === "Enter" && ask()}
    />
    <button class="tool" title="Attach a rulebook"><Icon name="paperclip" size={18} /></button>
    <button class="tool" title="Roll dice"><Icon name="dices" size={18} /></button>
    <button class="send-btn" disabled={!draft.trim()} onclick={() => ask()}>
      <Icon name="arrow-up" size={18} />
    </button>
  </div>
</div>
