<script lang="ts">
  import QueueNav from "$components/common/QueueNav.svelte";
  import type { HealthAction } from "$lib/health";
  import { splitLinks } from "$lib/links";
  import { placesShown } from "$lib/notices";
  import { notices } from "$stores/notices.svelte";

  /** The one notification window: errors and repository warnings, errors first. The warning
      window was the model — icon, `N of M`, arrows, close — and each entry brings its own
      buttons (doc/12-risks.md, R-178). */
  interface Props {
    oncopy: (text: string) => void;
    onopenurl: (url: string) => void;
    onshowoutput: (record: number) => void;
    onaction: (action: HealthAction) => void;
  }

  let { oncopy, onopenurl, onshowoutput, onaction }: Props = $props();

  const notice = $derived(notices.current);
  /** The entry whose list of places was opened past the first few. */
  let expandedFor = $state<string | null>(null);
  let copied = $state(false);

  function copy(text: string) {
    oncopy(text);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

{#if notice}
  {@const expanded = expandedFor === notice.key}
  {@const places = placesShown(notice.places ?? [], expanded)}
  <section
    class="notice {notice.severity}"
    role="alert"
    aria-label={notice.title}
  >
    <header>
      <span class="icon" aria-hidden="true">
        {#if notice.severity === "error"}
          <svg viewBox="0 0 16 16"
            ><circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.5" /><path
              d="M8 4.5v4.5M8 11v.5"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
            /></svg
          >
        {:else if notice.severity === "info"}
          <svg viewBox="0 0 16 16"
            ><circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.5" /><path
              d="M5.2 8.2 7.2 10.2 10.8 6"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
            /></svg
          >
        {:else}
          <svg viewBox="0 0 16 16"
            ><path
              d="M8 1.8 15 14.2H1Z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.4"
              stroke-linejoin="round"
            /><path
              d="M8 6.2v3.8M8 11.8v.4"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
            /></svg
          >
        {/if}
      </span>
      <span class="title">
        {notice.title}
        {#if notice.repeats > 1}
          <span class="repeats" title="The same failure, this many times">×{notice.repeats}</span>
        {/if}
      </span>
      <span class="side">
        <QueueNav
          at={notices.at}
          total={notices.all.length}
          noun="notification"
          onstep={(delta) => notices.step(delta)}
        />
        <button
          type="button"
          class="close"
          title={notice.warning ? "Remind me later" : "Close"}
          aria-label="Close"
          onclick={() => notices.dismiss()}>✕</button
        >
      </span>
    </header>

    <div class="body">
      <p class="text">{notice.body}</p>

      {#if notice.output}
        <!-- On one line: a <pre> keeps every space between the tags. -->
        <pre class="output mono">{#each splitLinks(notice.output) as part, index (index)}{#if part.href}<a class="url" href={part.href} title={part.href} onclick={(event) => { event.preventDefault(); onopenurl(part.href ?? ""); }}>{part.text}</a>{:else}{part.text}{/if}{/each}</pre>
      {:else if notice.outputLines}
        <p class="hint">{notice.outputLines} lines of output — Show Output reads them all.</p>
      {/if}

      {#if places.shown.length > 0}
        <ul class="places" class:scroll={expanded}>
          {#each places.shown as place (place.label)}
            <li>
              <span class="place">{place.label}</span>
              {#if place.detail}<span class="mono detail">{place.detail}</span>{/if}
            </li>
          {/each}
        </ul>
        {#if places.more > 0}
          <button type="button" class="link" onclick={() => (expandedFor = notice.key)}>
            and {places.more} more
          </button>
        {/if}
      {/if}

      {#if notice.fixes && notice.fixes.length > 0}
        <p class="hint">
          To fix it, run {(notice.places?.length ?? 0) > 1 ? "in each one listed" : "there"}:
        </p>
        {#each notice.fixes as fix (fix)}
          <div class="fix">
            <code class="mono">{fix}</code>
            <button type="button" class="btn" onclick={() => oncopy(fix)}>Copy</button>
          </div>
        {/each}
      {/if}

      {#if notice.docs}
        <button type="button" class="link" onclick={() => onopenurl(notice.docs ?? "")}>
          What this means — Git documentation
        </button>
      {/if}
    </div>

    <footer>
      {#if notice.action}
        {@const action = notice.action}
        <button type="button" class="btn" onclick={() => onaction(action)}>{action.label}</button>
      {/if}
      <span class="grow"></span>
      {#if notice.record !== undefined}
        {@const record = notice.record}
        <button type="button" class="btn" onclick={() => onshowoutput(record)}>Show Output</button>
      {/if}
      {#if notice.warning}
        <button type="button" class="btn" onclick={() => notices.dismiss()}>Remind me later</button>
        <button type="button" class="btn" onclick={() => void notices.ignore()}
          >Ignore for this repository</button
        >
      {:else}
        <button type="button" class="btn" onclick={() => copy(notice.report)}
          >{copied ? "Copied" : "Copy"}</button
        >
      {/if}
    </footer>
  </section>
{/if}

<style>
  .notice {
    position: fixed;
    right: var(--sp-5);
    bottom: calc(var(--h-statusbar) + var(--sp-5));
    z-index: 25;
    display: flex;
    flex-direction: column;
    width: min(480px, calc(100vw - 2 * var(--sp-5)));
    max-height: min(60vh, 520px);
    background: var(--surface-panel);
    border: 1px solid var(--divider);
    border-left: 3px solid var(--notice-accent);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
  }

  .notice.warning {
    --notice-accent: var(--status-modify);
  }

  .notice.error {
    --notice-accent: var(--status-delete);
  }

  .notice.info {
    --notice-accent: var(--status-add);
  }

  /* Three zones: the icon, a title that wraps inside its own column, and a side that keeps
     its width — the title can never run under the counter or the close button. */
  header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: start;
    gap: var(--sp-3);
    flex: none;
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
    border-radius: var(--r-md) var(--r-md) 0 0;
  }

  .icon {
    display: flex;
    align-items: center;
    height: var(--h-button-sm);
    color: var(--notice-accent);
  }

  .icon svg {
    width: 16px;
    height: 16px;
  }

  .title {
    align-self: center;
    font-weight: 600;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .repeats {
    margin-left: var(--sp-2);
    color: var(--notice-accent);
    font-size: var(--fs-header);
    font-weight: 400;
  }

  .side {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    white-space: nowrap;
  }

  .close {
    height: var(--h-button-sm);
    padding: 0 var(--sp-2);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

  .close:hover {
    color: var(--text-primary);
  }

  .body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding: var(--sp-4);
    font-size: var(--fs-dense);
    line-height: 1.45;
  }

  .body p {
    margin: 0 0 var(--sp-3);
  }

  .text {
    overflow-wrap: anywhere;
    user-select: text;
  }

  .url {
    color: var(--link);
  }

  .output {
    max-height: 12em;
    margin: 0 0 var(--sp-3);
    padding: var(--sp-2) var(--sp-3);
    overflow: auto;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .places {
    margin: 0 0 var(--sp-2);
    padding: 0;
    list-style: none;
  }

  .places.scroll {
    max-height: 9em;
    overflow-y: auto;
  }

  .places li {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-3);
  }

  .place {
    font-weight: 600;
  }

  .detail {
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }

  .hint {
    color: var(--text-secondary);
  }

  .fix {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-bottom: var(--sp-2);
  }

  .fix code {
    flex: 1 1 auto;
    padding: var(--sp-1) var(--sp-3);
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
  }

  .link {
    display: block;
    margin: 0 0 var(--sp-3);
    padding: 0;
    background: none;
    border: 0;
    color: var(--link);
    font: inherit;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  footer {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--divider);
  }

  .grow {
    flex: 1 1 auto;
  }

  .btn {
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    white-space: nowrap;
    cursor: default;
  }

  .btn:hover {
    border-color: var(--status-ref);
  }
</style>
