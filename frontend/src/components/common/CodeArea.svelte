<script lang="ts">
  import type { ConfigToken } from "$lib/config-syntax";

  interface LineMark {
    cls: string;
    title: string;
  }

  interface Props {
    text: string;
    label: string;
    tokenize: (line: string) => ConfigToken[];
    markOf?: (line: number) => LineMark | null;
    field?: HTMLTextAreaElement;
  }

  let { text = $bindable(), label, tokenize, markOf = () => null, field = $bindable() }: Props = $props();

  let top = $state(0);
  let left = $state(0);

  const lines = $derived(text.split("\n"));
</script>

<div class="editor">
  <div class="gutter mono">
    <div style:transform="translateY({-top}px)">
      {#each lines as _, at (at)}
        {@const mark = markOf(at + 1)}
        <div class={mark?.cls} title={mark?.title}>{at + 1}</div>
      {/each}
    </div>
  </div>
  <div class="stack">
    <pre class="paint mono" aria-hidden="true" style:transform="translate({-left}px, {-top}px)">{#each lines as line, at (at)}<div
          class="line {markOf(at + 1)?.cls ?? ''}">{#each tokenize(line) as token, index (index)}<span class={token.cls}
              >{token.text}</span
            >{/each}</div>{/each}</pre>
    <textarea
      bind:this={field}
      class="mono"
      spellcheck="false"
      wrap="off"
      aria-label={label}
      bind:value={text}
      onscroll={(event) => {
        top = event.currentTarget.scrollTop;
        left = event.currentTarget.scrollLeft;
      }}
    ></textarea>
  </div>
</div>

<style>
  .editor {
    display: flex;
    flex: 1 1 0;
    min-height: 0;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    background: var(--surface-input);
    overflow: hidden;
  }

  .gutter {
    flex: 0 0 auto;
    min-width: 40px;
    padding: var(--sp-4) var(--sp-3);
    overflow: hidden;
    color: var(--text-secondary);
    text-align: right;
    border-right: 1px solid var(--divider);
    user-select: none;
  }

  .gutter .bad {
    color: var(--status-delete);
    font-weight: 700;
  }

  .gutter .duplicate,
  .gutter .unused {
    color: var(--status-modify);
    font-weight: 700;
  }

  .stack {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
  }

  .gutter,
  .paint,
  textarea {
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    line-height: var(--lh-code);
    tab-size: 4;
  }

  .gutter > div > div,
  .line {
    height: var(--lh-code);
  }

  .paint {
    position: absolute;
    inset: 0 auto auto 0;
    margin: 0;
    padding: var(--sp-4);
    color: var(--text-code);
    white-space: pre;
    pointer-events: none;
  }

  .line.bad {
    background: var(--diff-del-line);
  }

  .line.duplicate,
  .line.unused {
    opacity: 0.6;
  }

  textarea {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    padding: var(--sp-4);
    background: transparent;
    color: transparent;
    caret-color: var(--text-primary);
    border: 0;
    outline: none;
    resize: none;
    white-space: pre;
    overflow: auto;
  }

  textarea::selection {
    background: var(--bg-selected);
    color: var(--text-primary);
  }
</style>
