<script>
  export let error;
  export let expression;
  export let answer;
  export let history;
  export let t;
  export let calc;
  export let saveCalculatorHistory;
  export let message;
</script>

      <section class="card calculator"><form on:submit|preventDefault={calc}><label>{t('Expression')}<input bind:value={expression} inputmode="decimal" placeholder="(24 + 36) / 2" autocomplete="off"/></label><output aria-live="polite">{answer || '0'}</output><div class="keypad">{#each ['(',')','%','/','7','8','9','*','4','5','6','-','1','2','3','+','0','.','^','='] as key}<button type="button" aria-label={t(({'+':'Add','-':'Subtract','*':'Multiply','/':'Divide','^':'Power','%':'Percent','=':'Calculate','(':'Open bracket',')':'Close bracket'})[key] || key)} on:click={() => key === '=' ? calc() : expression += key}>{key}</button>{/each}</div><div class="toolbar"><button class="primary">{t('Calculate')}</button><button type="button" on:click={() => expression = expression.slice(0, -1)}>{t('Backspace')}</button><button type="button" on:click={() => { expression = ''; answer = ''; }}>{t('Clear')}</button></div></form></section><section class="card"><h2>{t('History')}</h2><button disabled={!history.length} on:click={async () => { history = []; try { await saveCalculatorHistory(); error = ""; } catch (e) { error = message(e); } }}>{t('Clear history')}</button>{#each history as row}<button class="history" on:click={() => expression = row.expression}>{row.expression} = {row.answer}</button>{/each}</section>
