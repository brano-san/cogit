<script lang="ts">
  import { placeTip, STASH, TIP_DELAY_MS, watchTitles, type Placement } from "$lib/tooltip";

  /** The one tooltip of the window. It answers every `title` and every `data-tip` (what
      `Tooltip` sets), so all of them look alike and wait the same time (R-181). Every title is
      moved aside the moment it is in the page (`watchTitles`), so the browser never has one
      to draw a second, native tooltip from. The nearest titled ancestor's text joins the
      bubble as context (a row around its tag). */
  const SELECTOR = `[title], [data-tip], [${STASH}]`;

  interface Shown {
    text: string;
    /** The titled element around the one pointed at: a row around its tag. */
    context: string | null;
    hint: string | null;
    anchor: DOMRect;
    below: boolean;
  }

  let shown = $state.raw<Shown | null>(null);
  let placed = $state.raw<Placement | null>(null);
  let bubble: HTMLElement | undefined = $state();
  let current: Element | null = null;
  /** The element pointed at and its titled ancestors, `current` first. */
  let held: Element[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;
  /** A title rewritten while the bubble is up (a live status) shows its new text. */
  const watcher =
    typeof MutationObserver === "undefined"
      ? null
      : new MutationObserver(() => {
          if (shown) show();
        });

  function anchorOf(node: EventTarget | null): Element | null {
    return node instanceof Element ? node.closest(SELECTOR) : null;
  }

  function arm(element: Element | null) {
    if (element === current) return;
    disarm();
    if (!element) return;
    current = element;
    for (let node: Element | null = element; node; node = node.parentElement?.closest(SELECTOR) ?? null) {
      held.push(node);
      watcher?.observe(node, { attributes: true, attributeFilter: [STASH, "data-tip"] });
    }
    timer = setTimeout(show, TIP_DELAY_MS);
  }

  function hide() {
    clearTimeout(timer);
    shown = null;
    placed = null;
  }

  function disarm() {
    hide();
    current = null;
    watcher?.disconnect();
    held = [];
  }

  function textOf(element: Element | undefined): string {
    return (element?.getAttribute("data-tip") ?? element?.getAttribute(STASH) ?? element?.getAttribute("title") ?? "").trim();
  }

  function show() {
    const element = current;
    if (!element?.isConnected) return;
    const text = textOf(element);
    const around = held.slice(1).map(textOf).find((each) => each !== "") ?? null;
    if (text === "" && around === null) {
      hide();
      return;
    }
    if (import.meta.env.DEV && text !== "" && around !== null && text === around) {
      console.warn("tooltip: an element repeats the title of the one around it", element);
    }
    shown = {
      text: text || (around ?? ""),
      context: text !== "" && around !== text ? around : null,
      hint: element.getAttribute("data-tip-hint"),
      anchor: element.getBoundingClientRect(),
      below: element.hasAttribute("data-tip-below"),
    };
  }

  $effect(() => {
    if (!shown || !bubble) return;
    placed = placeTip(
      shown.anchor,
      { width: bubble.offsetWidth, height: bubble.offsetHeight },
      { width: window.innerWidth, height: window.innerHeight },
      shown.below,
    );
  });

  $effect(() => {
    const stop = watchTitles(document);
    return () => {
      stop();
      disarm();
    };
  });
</script>

<svelte:document
  onpointerover={(event) => arm(anchorOf(event.target))}
  onpointerout={(event) => {
    if (event.relatedTarget === null) disarm();
  }}
  onpointerdown={hide}
  onfocusin={(event) => {
    const element = anchorOf(event.target);
    if (element?.hasAttribute("data-tip")) arm(element);
  }}
  onfocusout={hide}
  onkeydown={(event) => event.key === "Escape" && hide()}
  onscrollcapture={hide}
/>
<svelte:window onblur={disarm} />

{#if shown}
  <div
    bind:this={bubble}
    class="bubble"
    role="tooltip"
    style:left="{placed?.left ?? 0}px"
    style:top="{placed?.top ?? 0}px"
    style:visibility={placed ? "visible" : "hidden"}
  >
    <span class="text">{shown.text}</span>{#if shown.hint}<span class="hint">{shown.hint}</span>{/if}{#if shown.context}<span
        class="context">{shown.context}</span
      >{/if}
  </div>
{/if}

<style>
  .bubble {
    position: fixed;
    z-index: 100;
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
    align-items: baseline;
    max-width: 360px;
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
    font-family: var(--font-ui);
    font-size: var(--fs-header);
    font-weight: 400;
    line-height: 1.4;
    letter-spacing: 0;
    text-transform: none;
    pointer-events: none;
  }

  .text {
    white-space: pre-line;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 14;
    line-clamp: 14;
    overflow: hidden;
  }

  /* What the element around it says (the row of a tag): one bubble, the pointed thing first. */
  .context {
    flex: 1 0 100%;
    padding-top: var(--sp-2);
    border-top: 1px solid var(--divider);
    color: var(--text-secondary);
    white-space: pre-line;
    overflow-wrap: anywhere;
  }

  .hint {
    flex: none;
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }
</style>
