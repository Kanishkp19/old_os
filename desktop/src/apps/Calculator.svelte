<script>
  export let error = '';
  export let expression = '';
  export let answer = '';
  export let history = [];
  export let t = (k) => k;
  export let calc;
  export let saveCalculatorHistory;
  export let message = (e) => e?.message || String(e);
  export let active = true;

  let showHistory = false;
  let copied = false;
  let copyTimer = null;
  let rootEl;

  // Track whether '=' was just evaluated so new digits start a clean calculation
  let justCalculated = false;

  async function copyResult() {
    const valueToCopy = answer || expression || '0';
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(valueToCopy);
      }
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => { copied = false; }, 1500);
    } catch {
      // Fallback or ignore
    }
  }

  function handleClear() {
    expression = '';
    answer = '';
    error = '';
    justCalculated = false;
  }

  function handleBackspace() {
    error = '';
    if (justCalculated || (answer && !expression)) {
      answer = '';
      justCalculated = false;
      return;
    }
    if (expression.length > 0) {
      expression = expression.slice(0, -1);
    }
  }

  function handleNumber(digit) {
    error = '';
    if (justCalculated) {
      expression = digit;
      answer = '';
      justCalculated = false;
    } else {
      expression += digit;
    }
  }

  function handleDecimal() {
    error = '';
    if (justCalculated) {
      expression = '0.';
      answer = '';
      justCalculated = false;
      return;
    }
    const parts = expression.split(/[\+\-\×\÷\*\/\(\)\^\%]/);
    const lastPart = parts[parts.length - 1] || '';
    if (!lastPart.includes('.')) {
      expression += (lastPart === '' ? '0.' : '.');
    }
  }

  function handleParentheses() {
    error = '';
    if (justCalculated) {
      expression = '(';
      answer = '';
      justCalculated = false;
      return;
    }
    const openCount = (expression.match(/\(/g) || []).length;
    const closeCount = (expression.match(/\)/g) || []).length;
    const lastChar = expression.slice(-1);

    if (openCount > closeCount && (/\d|\)|\%|\./.test(lastChar))) {
      expression += ')';
    } else {
      if (/\d|\)/.test(lastChar)) {
        expression += '×(';
      } else {
        expression += '(';
      }
    }
  }

  function handleOperator(op) {
    error = '';
    if (justCalculated && answer) {
      expression = answer + op;
      answer = '';
      justCalculated = false;
      return;
    }
    if (!expression && (op === '−' || op === '-')) {
      expression = '-';
      return;
    }
    if (!expression) return;

    const lastChar = expression.slice(-1);
    if (/[\+\-\×\÷\*\/\^]/.test(lastChar) || lastChar === '−') {
      expression = expression.slice(0, -1) + op;
    } else {
      expression += op;
    }
  }

  function handlePercent() {
    error = '';
    if (justCalculated && answer) {
      expression = answer + '%';
      answer = '';
      justCalculated = false;
      return;
    }
    if (!expression) return;
    const lastChar = expression.slice(-1);
    if (/\d|\)/.test(lastChar)) {
      expression += '%';
    }
  }

  async function handleCalculate() {
    if (!expression && !answer) return;
    try {
      await calc();
      justCalculated = true;
    } catch (e) {
      error = message(e);
    }
  }

  async function handleClearHistory() {
    history = [];
    try {
      await saveCalculatorHistory();
      error = '';
    } catch (e) {
      error = message(e);
    }
  }

  function restoreHistory(row) {
    expression = row.expression;
    answer = row.answer;
    justCalculated = true;
    showHistory = false;
    error = '';
  }

  function onKeyDown(event) {
    if (!active) return;
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
      if (!rootEl?.contains(event.target)) return;
    }
    if (event.metaKey || event.ctrlKey || event.altKey) return;

    const key = event.key;
    if (key >= '0' && key <= '9') {
      event.preventDefault();
      handleNumber(key);
    } else if (key === '.') {
      event.preventDefault();
      handleDecimal();
    } else if (key === '+') {
      event.preventDefault();
      handleOperator('+');
    } else if (key === '-') {
      event.preventDefault();
      handleOperator('−');
    } else if (key === '*' || key.toLowerCase() === 'x') {
      event.preventDefault();
      handleOperator('×');
    } else if (key === '/') {
      event.preventDefault();
      handleOperator('÷');
    } else if (key === '(' || key === ')') {
      event.preventDefault();
      expression += key;
    } else if (key === '%') {
      event.preventDefault();
      handlePercent();
    } else if (key === '^') {
      event.preventDefault();
      handleOperator('^');
    } else if (key === 'Enter' || key === '=') {
      event.preventDefault();
      handleCalculate();
    } else if (key === 'Backspace') {
      event.preventDefault();
      handleBackspace();
    } else if (key === 'Escape') {
      event.preventDefault();
      if (showHistory) {
        showHistory = false;
      } else {
        handleClear();
      }
    } else if (key.toLowerCase() === 'c') {
      event.preventDefault();
      handleClear();
    }
  }

  $: displayValue = answer || expression || '0';
  $: formulaPreview = answer ? `${expression} =` : (expression ? expression : '');
  $: dynamicFontSize = displayValue.length > 16 ? '1.5rem' : displayValue.length > 12 ? '1.9rem' : displayValue.length > 8 ? '2.3rem' : '2.8rem';
</script>

<svelte:window on:keydown={onKeyDown} />

<div class="calculator-app" bind:this={rootEl} role="region" aria-label={t('Calculator')}>
  <!-- High-Contrast Digital Display Header -->
  <div class="calc-display-panel">
    <div class="display-top-row">
      <button
        type="button"
        class="history-toggle-btn"
        class:active={showHistory}
        on:click={() => showHistory = !showHistory}
        aria-label={t('History')}
        title={t('History')}
      >
        <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" width="16" height="16">
          <circle cx="10" cy="10" r="7.5" />
          <polyline points="10 6 10 10 13 12" />
        </svg>
      </button>

      <div class="formula-preview" aria-live="polite">
        {formulaPreview || ''}
      </div>
    </div>

    <!-- Primary Output Row (Click to Copy) -->
    <button
      type="button"
      class="display-output-btn"
      on:click={copyResult}
      aria-label={`${t('Expression')}: ${displayValue}. ${t('Click to copy')}`}
      title={t('Click to copy')}
    >
      <span class="output-text" style:font-size={dynamicFontSize}>
        {displayValue}
      </span>
      {#if copied}
        <span class="copied-pill" role="status">{t('Copied!')}</span>
      {/if}
    </button>

    {#if error}
      <div class="calc-error-banner" role="alert">
        <span>{error}</span>
      </div>
    {/if}
  </div>

  <!-- Color-Coded 4x5 Tactical Keypad -->
  <div class="calc-keypad" role="group" aria-label={t('Calculator')}>
    <!-- Row 1: Function Keys & Divide -->
    <button type="button" class="key key-fn" on:click={handleClear} aria-label={t('Clear')}>
      {expression || answer ? 'C' : 'AC'}
    </button>
    <button type="button" class="key key-fn" on:click={handleParentheses} aria-label={t('Parentheses')}>
      ( )
    </button>
    <button type="button" class="key key-fn" on:click={handlePercent} aria-label={t('Percent')}>
      %
    </button>
    <button type="button" class="key key-op" on:click={() => handleOperator('÷')} aria-label={t('Divide')}>
      ÷
    </button>

    <!-- Row 2: 7, 8, 9, Multiply -->
    <button type="button" class="key key-num" on:click={() => handleNumber('7')}>7</button>
    <button type="button" class="key key-num" on:click={() => handleNumber('8')}>8</button>
    <button type="button" class="key key-num" on:click={() => handleNumber('9')}>9</button>
    <button type="button" class="key key-op" on:click={() => handleOperator('×')} aria-label={t('Multiply')}>
      ×
    </button>

    <!-- Row 3: 4, 5, 6, Subtract -->
    <button type="button" class="key key-num" on:click={() => handleNumber('4')}>4</button>
    <button type="button" class="key key-num" on:click={() => handleNumber('5')}>5</button>
    <button type="button" class="key key-num" on:click={() => handleNumber('6')}>6</button>
    <button type="button" class="key key-op" on:click={() => handleOperator('−')} aria-label={t('Subtract')}>
      −
    </button>

    <!-- Row 4: 1, 2, 3, Add -->
    <button type="button" class="key key-num" on:click={() => handleNumber('1')}>1</button>
    <button type="button" class="key key-num" on:click={() => handleNumber('2')}>2</button>
    <button type="button" class="key key-num" on:click={() => handleNumber('3')}>3</button>
    <button type="button" class="key key-op" on:click={() => handleOperator('+')} aria-label={t('Add')}>
      +
    </button>

    <!-- Row 5: 0, Decimal, Backspace, Equals -->
    <button type="button" class="key key-num" on:click={() => handleNumber('0')}>0</button>
    <button type="button" class="key key-num" on:click={handleDecimal} aria-label={t('Decimal')}>.</button>
    <button type="button" class="key key-action" on:click={handleBackspace} aria-label={t('Backspace')}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="19" height="19" aria-hidden="true">
        <path d="M21 4H8l-7 8 7 8h13a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2z" />
        <line x1="18" y1="9" x2="12" y2="15" />
        <line x1="12" y1="9" x2="18" y2="15" />
      </svg>
    </button>
    <button type="button" class="key key-equals" on:click={handleCalculate} aria-label={t('Calculate')}>
      =
    </button>
  </div>

  <!-- Smooth History Tape Overlay -->
  {#if showHistory}
    <div class="calc-history-overlay" role="dialog" aria-modal="true" aria-label={t('History')}>
      <div class="history-header">
        <div class="history-title-wrap">
          <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" width="16" height="16" aria-hidden="true">
            <circle cx="10" cy="10" r="7.5" />
            <polyline points="10 6 10 10 13 12" />
          </svg>
          <span class="history-title">{t('History')}</span>
        </div>
        <button
          type="button"
          class="history-close-btn"
          on:click={() => showHistory = false}
          aria-label={t('Close')}
        >
          &times;
        </button>
      </div>

      <div class="history-list">
        {#each history as row}
          <button type="button" class="history-row" on:click={() => restoreHistory(row)}>
            <div class="hist-formula">{row.expression} =</div>
            <div class="hist-result">{row.answer}</div>
          </button>
        {:else}
          <div class="history-empty">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="32" height="32" aria-hidden="true">
              <circle cx="12" cy="12" r="9" />
              <polyline points="12 7 12 12 15 15" />
            </svg>
            <p>{t('No calculations yet')}</p>
          </div>
        {/each}
      </div>

      <div class="history-footer">
        <button
          type="button"
          class="clear-history-btn"
          disabled={!history.length}
          on:click={handleClearHistory}
        >
          {t('Clear history')}
        </button>
        <button
          type="button"
          class="back-btn"
          on:click={() => showHistory = false}
        >
          {t('Back to calculator')}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .calculator-app {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 400px;
    display: flex;
    flex-direction: column;
    background: #11141A;
    background-image: radial-gradient(ellipse at 50% 0%, rgba(79, 163, 209, 0.08) 0%, transparent 70%);
    color: #FFFFFF;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    user-select: none;
    overflow: hidden;
    box-sizing: border-box;
  }

  .calc-display-panel {
    display: flex;
    flex-direction: column;
    padding: 14px 16px 10px 16px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.03) 0%, transparent 100%);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .display-top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 28px;
    margin-bottom: 4px;
  }

  .history-toggle-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.7);
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 120ms ease;
    padding: 0;
  }

  .history-toggle-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    color: #FFFFFF;
  }

  .history-toggle-btn.active {
    background: rgba(79, 163, 209, 0.25);
    color: #4FA3D1;
    border-color: rgba(79, 163, 209, 0.4);
  }

  .formula-preview {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.45);
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 260px;
    min-height: 18px;
  }

  .display-output-btn {
    background: transparent;
    border: 0;
    padding: 2px 0 6px 0;
    margin: 0;
    width: 100%;
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    cursor: pointer;
    text-align: right;
    position: relative;
    border-radius: 8px;
    transition: opacity 120ms ease;
  }

  .display-output-btn:hover .output-text {
    opacity: 0.92;
  }

  .display-output-btn:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px rgba(79, 163, 209, 0.5);
  }

  .output-text {
    font-weight: 600;
    line-height: 1.1;
    color: #FFFFFF;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    width: 100%;
    display: block;
  }

  .copied-pill {
    position: absolute;
    left: 0;
    bottom: 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    background: var(--os-accent, #5FBF8F);
    color: #0C1E14;
    padding: 3px 8px;
    border-radius: 12px;
    box-shadow: 0 2px 8px rgba(95, 191, 143, 0.4);
    animation: pop-in 150ms ease-out;
  }

  @keyframes pop-in {
    from { opacity: 0; transform: scale(0.9); }
    to { opacity: 1; transform: scale(1); }
  }

  .calc-error-banner {
    margin-top: 4px;
    padding: 4px 8px;
    font-size: 12px;
    color: #FF7B72;
    background: rgba(248, 81, 73, 0.12);
    border: 1px solid rgba(248, 81, 73, 0.25);
    border-radius: 6px;
    text-align: right;
  }

  .calc-keypad {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    grid-template-rows: repeat(5, 1fr);
    gap: 8px;
    padding: 12px;
    align-content: stretch;
    box-sizing: border-box;
  }

  .key {
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 14px;
    font-family: inherit;
    cursor: pointer;
    user-select: none;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 70ms cubic-bezier(0.2, 0, 0, 1), background-color 120ms ease, box-shadow 120ms ease;
    padding: 0;
    outline: none;
    min-height: 48px;
  }

  .key:active {
    transform: scale(0.95);
  }

  .key:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px #4FA3D1;
  }

  .key-fn {
    background: rgba(255, 255, 255, 0.12);
    color: #ECEFEA;
    font-size: 1.15rem;
    font-weight: 600;
  }

  .key-fn:hover {
    background: rgba(255, 255, 255, 0.18);
  }

  .key-num {
    background: rgba(255, 255, 255, 0.06);
    color: #FFFFFF;
    font-size: 1.35rem;
    font-weight: 500;
  }

  .key-num:hover {
    background: rgba(255, 255, 255, 0.10);
  }

  .key-op {
    background: #4FA3D1;
    color: #FFFFFF;
    font-size: 1.45rem;
    font-weight: 500;
    border-color: rgba(255, 255, 255, 0.15);
  }

  .key-op:hover {
    background: #5DB3E2;
  }

  .key-action {
    background: rgba(255, 255, 255, 0.06);
    color: #ECEFEA;
  }

  .key-action:hover {
    background: rgba(255, 255, 255, 0.10);
  }

  .key-equals {
    background: var(--os-accent, #5FBF8F);
    color: #0C1E14;
    font-size: 1.6rem;
    font-weight: 600;
    border: none;
    box-shadow: 0 4px 12px rgba(95, 191, 143, 0.3);
  }

  .key-equals:hover {
    background: #6ED3A0;
    box-shadow: 0 6px 16px rgba(95, 191, 143, 0.45);
  }

  /* History Overlay */
  .calc-history-overlay {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: rgba(16, 20, 27, 0.96);
    backdrop-filter: blur(20px);
    display: flex;
    flex-direction: column;
    animation: slide-in 160ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes slide-in {
    from { opacity: 0; transform: translateY(-8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .history-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .history-title-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #ECEFEA;
  }

  .history-title {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .history-close-btn {
    background: transparent;
    border: 0;
    color: rgba(255, 255, 255, 0.6);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    transition: all 120ms ease;
  }

  .history-close-btn:hover {
    color: #FFFFFF;
    background: rgba(255, 255, 255, 0.08);
  }

  .history-list {
    flex: 1;
    overflow-y: auto;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.2) transparent;
  }

  .history-row {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    padding: 10px 14px;
    text-align: right;
    cursor: pointer;
    transition: all 120ms ease;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
  }

  .history-row:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(79, 163, 209, 0.4);
    transform: translateX(-2px);
  }

  .hist-formula {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    font-variant-numeric: tabular-nums;
  }

  .hist-result {
    font-size: 18px;
    font-weight: 600;
    color: #FFFFFF;
    font-variant-numeric: tabular-nums;
  }

  .history-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.35);
    gap: 10px;
    padding: 24px;
  }

  .history-empty p {
    margin: 0;
    font-size: 14px;
  }

  .history-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    gap: 10px;
  }

  .clear-history-btn {
    background: rgba(248, 81, 73, 0.12);
    border: 1px solid rgba(248, 81, 73, 0.25);
    color: #FF7B72;
    padding: 6px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .clear-history-btn:hover:not(:disabled) {
    background: rgba(248, 81, 73, 0.22);
  }

  .clear-history-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .back-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #ECEFEA;
    padding: 6px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .back-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    color: #FFFFFF;
  }
</style>
