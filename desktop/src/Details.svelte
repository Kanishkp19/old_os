<script>
  export let value;
  export let level = 0;
  export let t = key => key;
  const label = key => key.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
</script>
{#if value === null || value === undefined}<span class="muted">—</span>
{:else if Array.isArray(value)}
  <div class="details-array">{#each value as item}<div class="detail-item"><svelte:self value={item} level={level + 1} {t}/></div>{/each}</div>
{:else if typeof value === 'object' && level < 6}
  <dl class="details">{#each Object.entries(value) as [key, item]}<div><dt>{t(label(key))}</dt><dd><svelte:self value={item} level={level + 1} {t}/></dd></div>{/each}</dl>
{:else}<span>{typeof value === 'boolean' ? (value ? '✓' : '—') : t(String(value))}</span>{/if}
