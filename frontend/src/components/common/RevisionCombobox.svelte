<script lang="ts">
  import Caret from "$components/common/Caret.svelte";
  import { placePopup, type Box, type Placed } from "$lib/popup-place";
  import { filterRevisions, stepOption, type RevisionOption } from "$lib/revision-options";
  import { truncateMiddle } from "$lib/truncate";

  /** Searchable picker of a revision: groups of branches and tags, and any text typed (a
      branch, a tag, a short or full hash, `HEAD~2`). The popup is a fixed-position portal
      placed by `placePopup`, so a dialog's edge never clips it; at most `LIMIT` rows are
      drawn, the rest say how many more a longer search would show. With `free={false}` the
      text only searches, and the value is always one of `options`. */
  interface Props {
    value: string;
    options: readonly RevisionOption[];
    label: string;
    placeholder?: string;
    free?: boolean;
    disabled?: boolean;
    onchange?: (value: string) => void;
  }

  let {
    value = $bindable(""),
    options,
    label,
    placeholder,
    free = true,
    disabled = false,
    onchange,
  }: Props = $props();

  const LIMIT = 200;
  const POPUP_MAX = 320;
  /** Wide enough for a branch name before the middle is cut. */
  const LABEL_CHARS = 58;
  const id = `revision-${Math.random().toString(36).slice(2, 8)}`;

  let open = $state(false);
  let editing = $state(false);
  let text = $state("");
  let active = $state(-1);
  let anchor: HTMLElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();
  let popup: HTMLElement | undefined = $state();
  let placed = $state<Placed | null>(null);
  let width = $state(0);

  const matched = $derived(options.find((option) => option.value === value));
  const shown = $derived(editing ? text : (matched?.display ?? value));
  const found = $derived(filterRevisions(options, editing ? text : "", LIMIT));

  function choose(option: RevisionOption) {
    if (option.disabled) return;
    value = option.value;
    editing = false;
    open = false;
    onchange?.(value);
  }

  function typed(next: string) {
    editing = true;
    text = next;
    if (!open) width = anchor?.getBoundingClientRect().width ?? 0;
    open = true;
    active = free ? -1 : 0;
    if (free) {
      value = next;
      onchange?.(next);
    }
  }

  function show() {
    if (disabled) return;
    width = anchor?.getBoundingClientRect().width ?? 0;
    open = true;
    const at = found.options.findIndex((option) => option.value === value);
    active = at >= 0 ? at : -1;
  }

  function close() {
    open = false;
    editing = false;
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) show();
      else active = stepOption(found.options, active, event.key === "ArrowDown" ? 1 : -1);
    } else if (event.key === "Enter" && open && found.options[active]) {
      event.preventDefault();
      choose(found.options[active]!);
    } else if (event.key === "Escape" && open) {
      // The dialog behind answers Escape too unless it is told this one is spent.
      event.preventDefault();
      close();
    } else if (event.key === "Tab") {
      close();
    }
  }

  /** The popup is measured once it holds its rows, then placed inside the window. */
  $effect(() => {
    void found.rows.length;
    if (!open || !anchor || !popup) {
      placed = null;
      return;
    }
    const box = anchor.getBoundingClientRect();
    width = box.width;
    const anchorBox: Box = { left: box.left, top: box.top, right: box.right, bottom: box.bottom };
    placed = placePopup(
      anchorBox,
      { width: box.width, height: Math.min(popup.scrollHeight + 2, POPUP_MAX) },
      { width: window.innerWidth, height: window.innerHeight },
    );
  });

  $effect(() => {
    if (!open || active < 0) return;
    popup?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  });

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function outside(event: PointerEvent) {
    if (!open) return;
    const target = event.target as Node;
    if (!anchor?.contains(target) && !popup?.contains(target)) close();
  }
</script>

<svelte:window onpointerdown={outside} onresize={close} />

<div class="combo" class:disabled bind:this={anchor}>
  <input
    bind:this={input}
    type="text"
    role="combobox"
    class:muted={!editing && matched?.display !== undefined}
    aria-label={label}
    title={shown}
    aria-expanded={open}
    aria-controls="{id}-list"
    aria-activedescendant={open && active >= 0 ? `${id}-${active}` : undefined}
    aria-autocomplete="list"
    autocomplete="off"
    spellcheck="false"
    {placeholder}
    {disabled}
    value={shown}
    oninput={(event) => typed(event.currentTarget.value)}
    onfocus={(event) => event.currentTarget.select()}
    onblur={() => {
      if (!open) editing = false;
    }}
    onclick={() => (open ? null : show())}
    {onkeydown}
  />
  <button
    type="button"
    class="caret"
    tabindex="-1"
    aria-label="Show {label} list"
    {disabled}
    onmousedown={(event) => event.preventDefault()}
    onclick={() => {
      if (open) close();
      else {
        input?.focus();
        show();
      }
    }}><Caret {open} /></button
  >
</div>

{#if open}
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    id="{id}-list"
    class="popup"
    role="listbox"
    aria-label={label}
    use:portal
    bind:this={popup}
    style:visibility={placed ? "visible" : "hidden"}
    style:left="{placed?.left ?? 0}px"
    style:top="{placed?.top ?? 0}px"
    style:width="{width}px"
    style:max-height="{Math.min(placed?.maxHeight ?? POPUP_MAX, POPUP_MAX)}px"
    onmousedown={(event) => event.preventDefault()}
  >
    {#each found.rows as row (row.kind === "group" ? `g:${row.title}` : `o:${row.option.value}`)}
      {#if row.kind === "group"}
        <div class="group" role="presentation">{row.title}</div>
      {:else}
        {@const option = row.option}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          id="{id}-{row.index}"
          class="option"
          class:active={row.index === active}
          class:chosen={option.value === value}
          role="option"
          tabindex="-1"
          aria-selected={option.value === value}
          aria-disabled={option.disabled}
          data-index={row.index}
          title={option.label.length > LABEL_CHARS || option.reason ? `${option.label}${option.reason ? ` — ${option.reason}` : ""}` : undefined}
          onclick={() => choose(option)}
          onpointermove={() => {
            if (!option.disabled) active = row.index;
          }}
        >
          <span class="name" class:off={option.disabled}>{truncateMiddle(option.label, LABEL_CHARS)}</span>
          {#if option.reason}
            <span class="note">{option.reason}</span>
          {:else if option.hint}
            <span class="note mono">{option.hint}</span>
          {/if}
        </div>
      {/if}
    {/each}
    {#if found.options.length === 0}
      <div class="empty">{free && editing && text.trim() !== "" ? "No match in the lists; the text is used as typed" : "Nothing matches"}</div>
    {/if}
    {#if found.hidden > 0}
      <div class="empty">{found.hidden} more; keep typing to narrow the list</div>
    {/if}
  </div>
{/if}

<style>
  .combo {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
  }

  input {
    width: 100%;
    height: var(--h-input);
    padding: 0 calc(var(--sp-5) + 12px) 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
  }

  input:hover:not(:disabled) {
    border-color: var(--state-focus-ring);
  }

  input:focus-visible {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: -1px;
  }

  input.muted {
    font-style: italic;
  }

  input:disabled {
    color: var(--text-secondary);
    opacity: 0.6;
  }

  .caret {
    position: absolute;
    right: var(--sp-2);
    display: flex;
    padding: var(--sp-2);
    background: none;
    border: 0;
    color: var(--text-secondary);
    cursor: default;
  }

  .popup {
    position: fixed;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
    font-size: var(--fs-dense);
  }

  .group {
    flex: none;
    padding: var(--sp-3) var(--sp-4) var(--sp-2);
    color: var(--text-muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .option {
    flex: none;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-4);
    min-height: var(--h-row);
    padding: var(--sp-2) var(--sp-4);
    color: var(--text-primary);
    cursor: default;
  }

  .option.chosen {
    background: var(--state-selected);
  }

  .option.active {
    background: var(--state-hover);
  }

  .name {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .name.off {
    color: var(--text-muted);
  }

  .note {
    flex: none;
    max-width: 55%;
    color: var(--text-muted);
    text-align: right;
    overflow-wrap: anywhere;
  }

  .note.mono {
    font-family: var(--font-mono);
  }

  .empty {
    padding: var(--sp-3) var(--sp-4);
    color: var(--text-muted);
  }
</style>
