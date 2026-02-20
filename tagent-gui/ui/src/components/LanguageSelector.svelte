<script lang="ts">
  import type { LanguageEntry } from "../lib/types";

  interface Props {
    value: string;
    languages: LanguageEntry[];
    allowAuto?: boolean;
    onchange?: (value: string) => void;
  }

  let { value = $bindable(), languages, allowAuto = true, onchange }: Props = $props();

  function handleChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    value = target.value;
    onchange?.(target.value);
  }
</script>

<select class="lang-select" bind:value onchange={handleChange}>
  {#each languages as lang}
    {#if lang.code !== "auto" || allowAuto}
      <option value={lang.code}>{lang.name}</option>
    {/if}
  {/each}
</select>

<style>
  .lang-select {
    background: var(--color-surface);
    color: var(--color-text);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 13px;
    cursor: pointer;
    outline: none;
    min-width: 130px;
  }

  .lang-select:hover {
    border-color: var(--color-accent);
  }

  .lang-select:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent) 20%, transparent);
  }
</style>
