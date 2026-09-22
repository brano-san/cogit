<script lang="ts">
  import { placeTip, TIP_DELAY_MS, type Placement } from "$lib/tooltip";

  /** The one tooltip of the window. It answers every `title` and every `data-tip` (what
      `Tooltip` sets), so all of them look alike and wait the same time (R-181). The title is
      moved aside while the pointer is over its element, which keeps the native one away. */
  const STASH = "data-cogit-title";
  const SELECTOR = `[title], [data-tip], [${STASH}]`;

  interface Shown {
    text: string;
    hint: string | null;
    anchor: DOMRect;
    below: boolean;
  }

  let shown = $state.raw<Shown | null>(null);
  let placed = $state.raw<Placement | null>(null);
  let bubble: HTMLElement | undefined = $state();
  let current: Element | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function anchorOf(node: EventTarget | null): Element | null {
    return node instanceof Element ? node.closest(SELECTOR) : null;
  }

  function arm(element: Element | null) {
    if (element === current) return;
    disarm();
    if (!element) return;
    current = element;
    const title = element.getAttribute("title");
    if (title !== null) {
      element.setAttribute(STASH, title);
      element.removeAttribute("title");
    }
    timer = setTimeout(show, TIP_DELAY_MS);
  }

  function disarm() {
    clearTimeout(timer);
    const element = current;
    current = null;
    shown = null;
    placed = null;
    if (!element) return;
    const stashed = element.getAttribute(STASH);
    element.removeAttribute(STASH);
    if (stashed !== null && !element.hasAttribute("title")) element.setAttribute("title", stashed);
  }

  function show() {
    const element = current;
    if (!element?.isConnected) return;
    const fresh = element.getAttribute("title");
    if (fresh !== null) {
      element.setAttribute(STASH, fresh);
      element.removeAttribute("title");
    }
    const text = element.getAttribute("data-tip") ?? element.getAttribute(STASH) ?? "";
    if (text.trim() === "") return;
    shown = {
      text,
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

  $effect(() => () => disarm());
</script>

<svelte:document
  onpointerover={(event) => arm(anchorOf(event.target))}
  onpointerout={(event) => {
    if (event.relatedTarget === null) disarm();
  }}
  onpointerdown={disarm}
  onfocusin={(event) => {
    const element = anchorOf(event.target);
    if (element?.hasAttribute("data-tip")) arm(element);
  }}
  onfocusout={disarm}
  onkeydown={(event) => event.key === "Escape" && disarm()}
  onscrollcapture={disarm}
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
    <span class="text">{shown.text}</span>{#if shown.hint}<span class="hint">{shown.hint}</span>{/if}
  </div>
{/if}

<style>
  .bubble {
    position: fixed;
    z-index: 100;
    display: flex;
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
  }

  .hint {
    flex: none;
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }
</style>
