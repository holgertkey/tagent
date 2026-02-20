<script lang="ts">
  import type { DictionaryData } from "../lib/types";

  interface Props {
    data: DictionaryData;
  }

  let { data }: Props = $props();
</script>

<div class="dict-root">
  {#each data.entries as entry}
    <div class="pos-block">
      <span class="pos-label">{entry.part_of_speech}</span>
      <ul class="defs">
        {#each entry.definitions as def}
          <li class="def-item">
            <span class="def-text">{def.text}</span>
            {#if def.synonyms.length > 0}
              <span class="synonyms"> [{def.synonyms.join(", ")}]</span>
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  {/each}
</div>

<style>
  .dict-root {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .pos-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .pos-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--color-accent);
  }

  .defs {
    margin: 0;
    padding-left: 14px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .def-item {
    font-size: 13px;
    line-height: 1.5;
    color: var(--color-text);
  }

  .synonyms {
    color: var(--color-text-secondary);
    font-size: 12px;
  }
</style>
