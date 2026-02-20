<script lang="ts">
  import { onMount } from "svelte";
  import LanguageSelector from "./components/LanguageSelector.svelte";
  import DictionaryView from "./components/DictionaryView.svelte";
  import {
    translate,
    getConfig,
    getLanguages,
    speak,
    swapLanguages,
    copyToClipboard,
  } from "./lib/tauri";
  import type { TranslationResult, ConfigData, LanguageEntry } from "./lib/types";

  // ─── State ──────────────────────────────────────────────────────────────────
  let inputText = $state("");
  let result = $state<TranslationResult | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);
  let speaking = $state(false);

  let fromLang = $state("auto");
  let toLang = $state("ru");

  let languages = $state<LanguageEntry[]>([]);
  let config = $state<ConfigData | null>(null);

  let textareaEl: HTMLTextAreaElement;

  // ─── Lifecycle ───────────────────────────────────────────────────────────────
  onMount(async () => {
    [languages, config] = await Promise.all([getLanguages(), getConfig()]);

    if (config) {
      // Convert language names to codes for the selectors
      fromLang = nameToCode(config.source_language, languages);
      toLang = nameToCode(config.target_language, languages);
    }

    textareaEl?.focus();
  });

  // ─── Helpers ─────────────────────────────────────────────────────────────────
  function nameToCode(name: string, langs: LanguageEntry[]): string {
    const found = langs.find(
      (l) => l.name.toLowerCase() === name.toLowerCase()
    );
    return found ? found.code : name.toLowerCase();
  }

  // ─── Actions ─────────────────────────────────────────────────────────────────
  async function doTranslate() {
    const text = inputText.trim();
    if (!text || loading) return;

    loading = true;
    error = null;
    result = null;
    copied = false;

    try {
      result = await translate(text, fromLang, toLang);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function doSwap() {
    try {
      const [newFrom, newTo] = await swapLanguages();
      fromLang = nameToCode(newFrom, languages);
      toLang = nameToCode(newTo, languages);
    } catch {
      // Fallback: swap locally
      if (fromLang !== "auto") {
        [fromLang, toLang] = [toLang, fromLang];
      }
    }
    // Re-translate if there's a result
    if (result) doTranslate();
  }

  async function doCopy() {
    const text = result?.translated ?? "";
    if (!text) return;
    try {
      await copyToClipboard(text);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (e) {
      error = `Copy failed: ${e}`;
    }
  }

  async function doSpeak() {
    const text = result?.translated ?? inputText.trim();
    if (!text || speaking) return;
    speaking = true;
    try {
      await speak(text);
    } catch {
      // TTS errors are non-critical
    } finally {
      speaking = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      doTranslate();
    }
  }

  function clearAll() {
    inputText = "";
    result = null;
    error = null;
    textareaEl?.focus();
  }

</script>

<div class="app">
  <!-- ─── Header ──────────────────────────────────────────────────────────── -->
  <header class="header">
    <div class="lang-row">
      <LanguageSelector bind:value={fromLang} {languages} allowAuto={true} />

      <button class="swap-btn" onclick={doSwap} title="Swap languages (⇄)">
        ⇄
      </button>

      <LanguageSelector bind:value={toLang} {languages} allowAuto={false} />
    </div>

    <div class="header-right">
      <span class="hotkey-hint">
        {config?.translate_hotkey ?? "Alt+Q"}
      </span>
    </div>
  </header>

  <!-- ─── Input area ────────────────────────────────────────────────────────── -->
  <section class="input-section">
    <div class="input-wrapper">
      <textarea
        bind:this={textareaEl}
        bind:value={inputText}
        onkeydown={handleKeydown}
        placeholder="Type text to translate… (Enter to translate, Shift+Enter for new line)"
        class="input-area"
        rows={5}
        spellcheck={false}
      ></textarea>

      {#if inputText}
        <button class="clear-btn" onclick={clearAll} title="Clear (Esc)">✕</button>
      {/if}
    </div>

    <div class="input-actions">
      <span class="char-count">{inputText.length}</span>

      <button
        class="action-btn secondary"
        onclick={doSpeak}
        disabled={speaking || !inputText.trim()}
        title="Speak input text"
      >
        {speaking ? "…" : "🔊"} Speak
      </button>

      <button
        class="action-btn primary"
        onclick={doTranslate}
        disabled={loading || !inputText.trim()}
      >
        {loading ? "Translating…" : "Translate ↵"}
      </button>
    </div>
  </section>

  <!-- ─── Output area ───────────────────────────────────────────────────────── -->
  <section class="output-section">
    {#if error}
      <div class="error-banner">⚠ {error}</div>
    {:else if loading}
      <div class="loading-state">
        <div class="spinner"></div>
        <span>Translating…</span>
      </div>
    {:else if result}
      <!-- Translation result -->
      <div class="result-header">
        <span class="result-label">
          {languages.find((l) => l.code === result!.from_lang)?.name ?? result.from_lang}
          →
          {languages.find((l) => l.code === result!.to_lang)?.name ?? result.to_lang}
        </span>
        <div class="result-actions">
          <button
            class="icon-btn"
            onclick={doSpeak}
            disabled={speaking}
            title="Speak translation"
          >
            {speaking ? "…" : "🔊"}
          </button>
          <button
            class="icon-btn"
            onclick={doCopy}
            title={copied ? "Copied!" : "Copy translation"}
            class:copied
          >
            {copied ? "✓" : "📋"}
          </button>
        </div>
      </div>

      <p class="translation-text">{result.translated}</p>

      {#if result.is_dictionary && result.dictionary}
        <div class="divider"></div>
        <DictionaryView data={result.dictionary} />
      {/if}
    {:else}
      <div class="empty-state">
        <span>Translation will appear here</span>
      </div>
    {/if}
  </section>

  <!-- ─── Status bar ────────────────────────────────────────────────────────── -->
  <footer class="statusbar">
    <span>Tagent v{__APP_VERSION__}</span>
    <span class="statusbar-right">
      {#if config?.save_translation_history}
        <span class="badge">history on</span>
      {/if}
      {#if config?.copy_to_clipboard}
        <span class="badge">auto-copy</span>
      {/if}
    </span>
  </footer>
</div>

<style>
  /* ─── Layout ───────────────────────────────────────────────────────────────── */
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 0;
  }

  /* ─── Header ────────────────────────────────────────────────────────────────── */
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    gap: 8px;
    flex-shrink: 0;
  }

  .lang-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .swap-btn {
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 16px;
    padding: 4px 8px;
    border-radius: 6px;
    transition: color 0.15s, background 0.15s;
  }

  .swap-btn:hover {
    background: var(--color-surface-hover);
    color: var(--color-accent);
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .hotkey-hint {
    font-size: 11px;
    color: var(--color-text-secondary);
    background: var(--color-bg);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    padding: 2px 7px;
    font-family: monospace;
  }

  /* ─── Input section ─────────────────────────────────────────────────────────── */
  .input-section {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    padding: 12px 14px;
    gap: 8px;
    border-bottom: 1px solid var(--color-border);
  }

  .input-wrapper {
    position: relative;
  }

  .input-area {
    width: 100%;
    background: var(--color-bg);
    color: var(--color-text);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 10px 34px 10px 12px;
    resize: none;
    font-size: 14px;
    line-height: 1.6;
    user-select: text;
    transition: border-color 0.15s;
  }

  .input-area::placeholder {
    color: var(--color-text-placeholder);
  }

  .clear-btn {
    position: absolute;
    top: 8px;
    right: 8px;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 13px;
    padding: 2px 5px;
    border-radius: 4px;
  }

  .clear-btn:hover {
    background: var(--color-surface-hover);
    color: var(--color-text);
  }

  .input-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .char-count {
    font-size: 11px;
    color: var(--color-text-secondary);
    margin-right: auto;
  }

  /* ─── Buttons ───────────────────────────────────────────────────────────────── */
  .action-btn {
    padding: 7px 16px;
    font-size: 13px;
    border-radius: var(--radius);
    font-weight: 500;
  }

  .action-btn.primary {
    background: var(--color-accent);
    color: #1e1e2e;
  }

  .action-btn.primary:hover:not(:disabled) {
    opacity: 0.9;
  }

  .action-btn.secondary {
    background: var(--color-surface);
    color: var(--color-text);
    border: 1px solid var(--color-border);
  }

  .action-btn.secondary:hover:not(:disabled) {
    background: var(--color-surface-hover);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .icon-btn {
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 15px;
    padding: 4px 8px;
    border-radius: 6px;
    transition: color 0.15s, background 0.15s;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--color-surface-hover);
    color: var(--color-text);
  }

  .icon-btn.copied {
    color: var(--color-success);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ─── Output section ────────────────────────────────────────────────────────── */
  .output-section {
    flex: 1;
    padding: 14px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    user-select: text;
  }

  .result-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .result-label {
    font-size: 12px;
    color: var(--color-text-secondary);
    font-weight: 500;
  }

  .result-actions {
    display: flex;
    gap: 4px;
  }

  .translation-text {
    font-size: 18px;
    font-weight: 500;
    color: var(--color-text);
    line-height: 1.4;
    word-break: break-word;
  }

  .divider {
    height: 1px;
    background: var(--color-border);
    margin: 4px 0;
  }

  /* ─── States ────────────────────────────────────────────────────────────────── */
  .empty-state,
  .loading-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--color-text-secondary);
    font-size: 13px;
  }

  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid var(--color-border);
    border-top-color: var(--color-accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .error-banner {
    background: color-mix(in srgb, var(--color-danger) 15%, transparent);
    color: var(--color-danger);
    border: 1px solid color-mix(in srgb, var(--color-danger) 40%, transparent);
    border-radius: var(--radius);
    padding: 10px 14px;
    font-size: 13px;
  }

  /* ─── Status bar ─────────────────────────────────────────────────────────────── */
  .statusbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px 14px;
    background: var(--color-surface);
    border-top: 1px solid var(--color-border);
    font-size: 11px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
  }

  .statusbar-right {
    display: flex;
    gap: 6px;
  }

  .badge {
    background: var(--color-accent-dim);
    color: var(--color-accent);
    border-radius: 3px;
    padding: 1px 6px;
    font-size: 10px;
  }
</style>
