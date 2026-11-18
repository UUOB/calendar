<script>
  // Picks an alert: one of the usual times, or any time you type in
  // ("Custom…": a number and minutes, hours or days). The value is a string:
  // minutes before the start, 'none', or 'default' (when `defaultLabel` is given).
  import { BEFORE, MAX_BEFORE, beforeLabel } from './lib/upcoming.js';

  let { value, onchange, defaultLabel = null, class: cls = 'field', label = 'Alert' } = $props();

  const UNITS = [['minutes', 1], ['hours', 60], ['days', 1440]];
  const preset = (v) => v === 'default' || v === 'none' || BEFORE.some((m) => m !== null && String(m) === v);
  let custom = $state(false); // typing a time of your own
  let n = $state(15);
  let per = $state(1);

  // Opening "Custom…" starts from the time chosen now, in its largest whole unit.
  function startCustom() {
    const m = Number(value);
    if (Number.isFinite(m) && m > 0) [n, per] = [...UNITS].reverse().map(([, u]) => [m / u, u]).find(([x]) => Number.isInteger(x));
    else [n, per] = [15, 1];
    custom = true;
  }
  function pick(v) {
    if (v === 'custom') return startCustom();
    custom = false;
    onchange(v);
  }
  function set() {
    const m = Math.round(Number(n) * per);
    if (!(m > 0)) return;
    onchange(String(Math.min(m, MAX_BEFORE)));
    custom = false;
  }
</script>

<select class={cls} aria-label={label} value={custom ? 'custom' : value} onchange={(e) => pick(e.currentTarget.value)}>
  {#if defaultLabel}<option value="default">Default ({defaultLabel})</option>{/if}
  {#each BEFORE as m}<option value={m === null ? 'none' : String(m)}>{m === null ? 'No alert' : beforeLabel(m)}</option>{/each}
  {#if !preset(value)}<option value={value}>{beforeLabel(Number(value))}</option>{/if}
  <option value="custom">Custom…</option>
</select>
{#if custom}
  <span class="custom" role="group" aria-label="Your own alert time">
    <input class="field n" type="number" min="1" max={Math.floor(MAX_BEFORE / per)} bind:value={n} aria-label="How many" onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), set())} />
    <select class="field" bind:value={per} aria-label="Unit">{#each UNITS as [name, u]}<option value={u}>{name}</option>{/each}</select>
    <span class="before">before</span>
    <button type="button" class="btn sm primary" onclick={set}>Set</button>
  </span>
{/if}

<style>
  .custom { display: flex; align-items: center; gap: 8px; margin-top: 8px; flex-wrap: wrap; }
  .custom .field { width: auto; }
  .custom .n { width: 76px; }
  .before { color: var(--text-2); font-size: 15px; }
</style>
