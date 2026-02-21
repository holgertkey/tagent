<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    translate,
    getConfig,
    getLanguages,
    speak,
    saveConfig,
    copyToClipboard,
  } from "./lib/tauri";
  import type { ConfigData, LanguageEntry } from "./lib/types";

  // ─── Types ────────────────────────────────────────────────────────────────

  type LineKind = "echo" | "result" | "error" | "info";

  interface Line {
    kind: LineKind;
    text: string;
  }

  // ─── State ────────────────────────────────────────────────────────────────

  let inputText = $state("");
  let lines = $state<Line[]>([]);
  let loading = $state(false);
  let config = $state<ConfigData | null>(null);
  let languages = $state<LanguageEntry[]>([]);
  let fromLang = $state("auto");
  let toLang = $state("ru");
  let lastResult = $state<string | null>(null);

  let inputEl: HTMLInputElement;
  let outputEl: HTMLDivElement;

  // ─── Derived ─────────────────────────────────────────────────────────────

  const promptLabel = $derived(
    fromLang === "auto"
      ? "[Auto]"
      : `[${languages.find((l) => l.code === fromLang)?.name ?? fromLang}]`
  );

  const translateHotkey = $derived(config?.translate_hotkey ?? "Alt+Q");
  const speechHotkey = $derived(config?.speech_hotkey ?? "Alt+E");

  // ─── Lifecycle ────────────────────────────────────────────────────────────

  onMount(async () => {
    [languages, config] = await Promise.all([getLanguages(), getConfig()]);
    if (config) {
      fromLang = nameToCode(config.source_language, languages);
      toLang = nameToCode(config.target_language, languages);
    }
    inputEl?.focus();
  });

  // ─── Helpers ─────────────────────────────────────────────────────────────

  function nameToCode(name: string, langs: LanguageEntry[]): string {
    const found = langs.find(
      (l) => l.name.toLowerCase() === name.toLowerCase()
    );
    return found ? found.code : name.toLowerCase();
  }

  function codeToName(code: string): string {
    if (code === "auto") return "Auto";
    return languages.find((l) => l.code === code)?.name ?? code;
  }

  function addLine(kind: LineKind, text: string) {
    lines = [...lines, { kind, text }];
  }

  async function scrollToBottom() {
    await tick();
    if (outputEl) outputEl.scrollTop = outputEl.scrollHeight;
  }

  // ─── Translation ──────────────────────────────────────────────────────────

  async function doTranslate(text: string) {
    if (!text || loading) return;
    loading = true;
    addLine("echo", text);

    try {
      const res = await translate(text, fromLang, toLang);

      if (res.is_dictionary && res.dictionary) {
        addLine("result", res.translated);
        addLine("info", "");
        for (const entry of res.dictionary.entries) {
          addLine("info", `  ${entry.part_of_speech}:`);
          for (const def of entry.definitions) {
            addLine("info", `    ${def.text}`);
            if (def.synonyms.length > 0) {
              addLine("info", `    (${def.synonyms.slice(0, 3).join(", ")})`);
            }
          }
        }
      } else {
        addLine("result", res.translated);
      }

      lastResult = res.translated;

      if (config?.copy_to_clipboard) {
        await copyToClipboard(res.translated).catch(() => {});
      }
    } catch (e) {
      addLine("error", `Error: ${e}`);
    } finally {
      loading = false;
      await scrollToBottom();
    }
  }

  // ─── Commands ─────────────────────────────────────────────────────────────

  async function handleCommand(cmd: string) {
    const parts = cmd.trim().split(/\s+/);
    const name = parts[0].toLowerCase();

    switch (name) {
      case "/h":
      case "/help":
        addLine("info", "Commands:");
        addLine("info", "  /h (help), /c (config), /s (speech), /l (lang),");
        addLine("info", "  /save (config), /cls (clear), /q (quit)");
        addLine("info", "");
        addLine("info", "Language examples:");
        addLine("info", "  /l ru            -- set target to Russian");
        addLine("info", "  /l en ru         -- source=English, target=Russian");
        addLine("info", "  /l auto          -- set source to Auto");
        break;

      case "/c":
      case "/config":
        if (config) {
          addLine("info", "Current config:");
          addLine(
            "info",
            `  Languages:  ${codeToName(fromLang)} \u2192 ${codeToName(toLang)}`
          );
          addLine("info", `  Provider:   ${config.translate_provider}`);
          addLine(
            "info",
            `  Translate:  ${config.translate_hotkey}`
          );
          addLine(
            "info",
            `  Speech:     ${config.speech_hotkey}`
          );
          addLine(
            "info",
            `  Dictionary: ${config.show_dictionary ? "on" : "off"}`
          );
          addLine(
            "info",
            `  History:    ${config.save_translation_history ? "on" : "off"}`
          );
          addLine(
            "info",
            `  Auto-copy:  ${config.copy_to_clipboard ? "on" : "off"}`
          );
        }
        break;

      case "/s":
      case "/speech": {
        const text = lastResult ?? "";
        if (text) {
          addLine("info", `Speaking: ${text}`);
          await speak(text).catch((e) => addLine("error", `TTS error: ${e}`));
        } else {
          addLine("info", "Nothing to speak. Translate something first.");
        }
        break;
      }

      case "/l":
      case "/lang": {
        const args = parts.slice(1);
        if (args.length === 0) {
          addLine(
            "info",
            `Languages: ${codeToName(fromLang)} \u2192 ${codeToName(toLang)}`
          );
          break;
        }
        if (args.length === 1) {
          // Set target (or source if "auto")
          const code = args[0].toLowerCase();
          if (code === "auto") {
            fromLang = "auto";
            addLine("info", "Source language: Auto");
            break;
          }
          const lang = languages.find(
            (l) => l.code === code || l.name.toLowerCase() === code
          );
          if (lang) {
            toLang = lang.code;
            addLine("info", `Target language: ${lang.name}`);
          } else {
            addLine("error", `Unknown language: ${args[0]}`);
          }
          break;
        }
        // Two args: source and target
        const srcCode = args[0].toLowerCase();
        const tgtCode = args[1].toLowerCase();
        const srcLang =
          srcCode === "auto"
            ? { code: "auto", name: "Auto" }
            : languages.find(
                (l) => l.code === srcCode || l.name.toLowerCase() === srcCode
              );
        const tgtLang = languages.find(
          (l) => l.code === tgtCode || l.name.toLowerCase() === tgtCode
        );
        if (srcLang && tgtLang) {
          fromLang = srcLang.code;
          toLang = tgtLang.code;
          addLine(
            "info",
            `Languages: ${srcLang.name} \u2192 ${tgtLang.name}`
          );
        } else {
          addLine("error", `Unknown language(s): ${args.join(" ")}`);
        }
        break;
      }

      case "/save":
        if (config) {
          try {
            await saveConfig({
              ...config,
              source_language: codeToName(fromLang),
              target_language: codeToName(toLang),
            });
            addLine("info", "Config saved.");
          } catch (e) {
            addLine("error", `Save failed: ${e}`);
          }
        }
        break;

      case "/cls":
      case "/clear":
        lines = [];
        break;

      case "/q":
      case "/quit":
        await getCurrentWindow().hide();
        break;

      default:
        addLine("error", `Unknown command: ${name}. Type /h for help.`);
    }

    await scrollToBottom();
  }

  // ─── Input handler ────────────────────────────────────────────────────────

  async function handleKeydown(e: KeyboardEvent) {
    if (e.key !== "Enter") return;
    e.preventDefault();
    const text = inputText.trim();
    inputText = "";
    if (!text) return;

    if (text.startsWith("/")) {
      addLine("echo", text);
      await handleCommand(text);
    } else {
      await doTranslate(text);
    }
  }

  function focusInput() {
    inputEl?.focus();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="terminal" onclick={focusInput}>

  <!-- ─── Banner ──────────────────────────────────────────────────────────── -->
  <div class="banner">
    <div class="banner-title">Text Translator v{__APP_VERSION__}</div>
    <div class="banner-section">
      <div class="banner-label">Active Hotkeys:</div>
      <div class="banner-item">Translation: <span class="key">{translateHotkey}</span></div>
      <div class="banner-item">Speech: <span class="key">{speechHotkey}</span></div>
    </div>
    <div class="banner-section">
      <div class="banner-label">Commands:</div>
      <div class="banner-item">/h (help), /c (config), /s (speech), /l (lang),</div>
      <div class="banner-item">/save (config), /cls (clear), /q (quit)</div>
    </div>
  </div>

  <div class="banner-sep"></div>

  <!-- ─── Output ──────────────────────────────────────────────────────────── -->
  <div class="output" bind:this={outputEl}>
    {#each lines as line}
      <div class="line line-{line.kind}">{line.text}</div>
    {/each}
    {#if loading}
      <div class="line line-info">  ...</div>
    {/if}
  </div>

  <!-- ─── Prompt ───────────────────────────────────────────────────────────── -->
  <div class="prompt-row">
    <span class="prompt-label">{promptLabel}:</span>
    <input
      bind:this={inputEl}
      bind:value={inputText}
      onkeydown={handleKeydown}
      class="prompt-input"
      spellcheck={false}
      autocomplete="off"
      disabled={loading}
    />
  </div>
</div>

<style>
  /* ─── Terminal container ─────────────────────────────────────────────────── */
  .terminal {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
    font-family: "Consolas", "Courier New", monospace;
    font-size: 13px;
    line-height: 1.55;
    color: var(--color-text);
    cursor: text;
    padding: 0;
  }

  /* ─── Banner ─────────────────────────────────────────────────────────────── */
  .banner {
    padding: 12px 16px 10px;
    flex-shrink: 0;
  }

  .banner-title {
    color: var(--color-accent);
    font-weight: bold;
    font-size: 14px;
    margin-bottom: 8px;
  }

  .banner-section {
    margin-bottom: 6px;
  }

  .banner-label {
    color: var(--color-text-secondary);
  }

  .banner-item {
    padding-left: 16px;
    color: var(--color-text-secondary);
  }

  .key {
    color: var(--color-warning);
  }

  .banner-sep {
    height: 1px;
    background: var(--color-border);
    margin: 0 0 4px;
    flex-shrink: 0;
  }

  /* ─── Output ─────────────────────────────────────────────────────────────── */
  .output {
    flex: 1;
    overflow-y: auto;
    padding: 6px 16px 4px;
    min-height: 0;
  }

  .line {
    white-space: pre-wrap;
    word-break: break-word;
    padding: 1px 0;
    user-select: text;
  }

  .line-echo {
    color: var(--color-text-secondary);
  }

  .line-echo::before {
    content: "> ";
    color: var(--color-text-placeholder);
  }

  .line-result {
    color: var(--color-success);
    padding-left: 16px;
    font-size: 14px;
  }

  .line-error {
    color: var(--color-danger);
    padding-left: 16px;
  }

  .line-info {
    color: var(--color-text-secondary);
    padding-left: 16px;
  }

  /* ─── Prompt ─────────────────────────────────────────────────────────────── */
  .prompt-row {
    display: flex;
    align-items: center;
    padding: 6px 16px 10px;
    flex-shrink: 0;
    gap: 0;
    border-top: 1px solid var(--color-border);
  }

  .prompt-label {
    color: var(--color-accent);
    white-space: nowrap;
    padding-right: 6px;
    font-family: inherit;
    font-size: inherit;
    flex-shrink: 0;
  }

  .prompt-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--color-text);
    font-family: inherit;
    font-size: inherit;
    line-height: inherit;
    caret-color: var(--color-accent);
    padding: 0;
  }

  .prompt-input:disabled {
    opacity: 0.5;
  }
</style>
