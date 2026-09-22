<script lang="ts">
  import QueueNav from "$components/common/QueueNav.svelte";
  import { health } from "$stores/health.svelte";

  /** SmartGit's repository warning: what is wrong, where, how to fix it, and two ways to
      make it go away. Closing it is "Remind me later" — nothing here changes the repository. */
  interface Props {
    oncopy: (text: string) => void;
    onopenurl: (url: string) => void;
  }

  let { oncopy, onopenurl }: Props = $props();

  const warning = $derived(health.current);
</script>

{#if warning}
  <section class="notice" role="alert" aria-label={warning.title}>
    <header>
      <span class="icon" aria-hidden="true">⚠</span>
      <span class="title">{warning.title}</span>
      <span class="grow"></span>
      <QueueNav
        at={health.at}
        total={health.warnings.length}
        noun="warning"
        onstep={(delta) => health.step(delta)}
      />
      <button type="button" class="close" title="Remind me later" onclick={() => health.remindLater()}
        >✕</button
      >
    </header>

    <div class="body">
      <p>{warning.body}</p>

      <ul class="places">
        {#each warning.places as place (place.label)}
          <li>
            <span class="place">{place.label}</span>
            {#if place.detail}<span class="mono detail">{place.detail}</span>{/if}
          </li>
        {/each}
      </ul>

      <p class="fix-heading">
        To fix it, run {warning.places.length > 1 ? "in each one listed" : "there"}:
      </p>
      {#each warning.fixes as fix (fix)}
        <div class="fix">
          <code class="mono">{fix}</code>
          <button type="button" class="btn" onclick={() => oncopy(fix)}>Copy</button>
        </div>
      {/each}

      <button type="button" class="link" onclick={() => onopenurl(warning.docs)}>
        What this means — Git documentation
      </button>
    </div>

    <footer>
      <button type="button" class="btn" onclick={() => health.remindLater()}>Remind me later</button>
      <button type="button" class="btn" onclick={() => void health.ignore()}
        >Ignore for this repository</button
      >
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
    border-left: 3px solid var(--status-modify);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: var(--h-panel-hdr);
    padding: var(--sp-2) var(--sp-4);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
    border-radius: var(--r-md) var(--r-md) 0 0;
  }

  .icon {
    color: var(--status-modify);
    font-size: 14px;
  }

  .title {
    font-weight: 600;
  }

  .grow {
    flex: 1 1 auto;
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
    overflow: auto;
    padding: var(--sp-4);
    font-size: var(--fs-dense);
    line-height: 1.45;
  }

  .body p {
    margin: 0 0 var(--sp-3);
  }

  .places {
    margin: 0 0 var(--sp-3);
    padding: 0;
    list-style: none;
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

  .fix-heading {
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
    margin-top: var(--sp-2);
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
    justify-content: flex-end;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--divider);
  }

  .btn {
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .btn:hover {
    border-color: var(--status-ref);
  }
</style>
