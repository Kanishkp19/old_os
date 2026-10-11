<script>
  import { invoke } from '@tauri-apps/api/core';

  let hindiInput = '';
  let token = '';
  let connection = 'Not checked';
  let latency = null;
  let overview = '';
  let memory = 'Not measured';

  async function readOverview() {
    connection = 'Checking';
    latency = null;
    overview = '';
    const start = performance.now();
    try {
      const response = await fetch('http://127.0.0.1:47801/api/overview', {
        headers: { Authorization: `Bearer ${token.trim()}` },
        cache: 'no-store'
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const body = await response.json();
      latency = Math.round(performance.now() - start);
      connection = 'Connected';
      overview = JSON.stringify(body).slice(0, 400);
    } catch (error) {
      connection = `Failed: ${error.message}`;
    }
  }

  async function reportMemory() {
    const status = await invoke('get_smoke_status');
    const heap = performance.memory;
    memory = heap
      ? `JavaScript heap ${Math.round(heap.usedJSHeapSize / 1024)} KiB; process uptime ${status.uptime_ms} ms`
      : `WebKit does not expose JavaScript heap metrics here; process uptime ${status.uptime_ms} ms. Use guest smaps_rollup for RSS/PSS.`;
  }
</script>

<main>
  <h1>Home Hub OS Smoke Test</h1>
  <label for="hindi">Hindi / IME input</label>
  <input id="hindi" bind:value={hindiInput} placeholder="हिन्दी यहाँ लिखें" />
  <p>Composed text: {hindiInput}</p>

  <label for="token">Guest-local proxy bearer token</label>
  <input id="token" type="password" bind:value={token} autocomplete="off" />
  <button on:click={readOverview} disabled={!token.trim()}>Read overview</button>
  <p class:connected={connection === 'Connected'}>{connection}{latency === null ? '' : ` in ${latency} ms`}</p>
  {#if overview}<pre>{overview}</pre>{/if}

  <button on:click={reportMemory}>Report WebKit memory</button>
  <p>{memory}</p>
</main>

<style>
  :global(body) { font-family: system-ui, sans-serif; background: #f5f7f8; color: #152522; margin: 0; }
  main { max-width: 42rem; margin: 2rem auto; padding: 2rem; background: white; border-radius: 1rem; }
  h1 { font-size: 1.4rem; background: #163d36; color: white; padding: .8rem 1rem; border-radius: .5rem; }
  label { display: block; margin-top: 1.2rem; font-weight: 600; }
  input { display: block; width: 100%; box-sizing: border-box; padding: .7rem; margin-top: .3rem; }
  button { margin: 1rem .5rem 0 0; padding: .7rem 1rem; cursor: pointer; }
  .connected { color: #16713d; font-weight: 700; }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
