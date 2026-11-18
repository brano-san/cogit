<script lang="ts">
  import { ciSummary, ciTooltip } from "$lib/ci-status";
  import type { CiStatus } from "$lib/ipc";
  import { ciStatus } from "$stores/ci-status.svelte";

  interface Props {
    status: CiStatus;
  }

  let { status }: Props = $props();
</script>

<button
  type="button"
  class="ci {status.state}"
  tabindex="-1"
  aria-label={ciSummary(status)}
  data-tip={ciTooltip(status)}
  onclick={(event) => {
    event.stopPropagation();
    void ciStatus.openMenu(status, event.clientX, event.clientY);
  }}
  ondblclick={(event) => event.stopPropagation()}
>
  <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    {#if status.state === "success"}
      <path d="M3.5 8.4l3 3 6-6.6" />
    {:else if status.state === "failure"}
      <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
    {:else if status.state === "pending"}
      <circle cx="8" cy="8" r="3" fill="currentColor" stroke="none" />
    {:else}
      <path d="M4.5 8h7" />
    {/if}
  </svg>
</button>

<style>
  .ci {
    display: inline-flex;
    flex: none;
    align-items: center;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .ci.success {
    color: var(--status-add);
  }

  .ci.failure {
    color: var(--status-delete);
  }

  .ci.pending {
    color: var(--status-modify);
  }
</style>
