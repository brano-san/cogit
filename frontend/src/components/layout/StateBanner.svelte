<script lang="ts">
  import type { Banner, BannerAction } from "$lib/repo-state";

  interface Props {
    banner: Banner;
    busy?: boolean;
    onaction: (action: BannerAction) => void;
  }

  let { banner, busy = false, onaction }: Props = $props();

  const LABELS: Record<BannerAction, string> = {
    continue: "Continue",
    skip: "Skip",
    abort: "Abort",
    createBranch: "Create Branch",
  };
</script>

<div class="banner {banner.severity}">
  <span class="title">{banner.title}</span>
  <span class="detail truncate" title={banner.detail}>{banner.detail}</span>
  {#each banner.actions as action (action)}
    <button type="button" disabled={busy} onclick={() => onaction(action)}>{LABELS[action]}</button>
  {/each}
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-5);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .banner.info {
    background: var(--surface-raised);
  }

  .banner.warning {
    background: var(--c-modified-bg);
  }

  .banner.error {
    background: var(--c-deleted-bg);
  }

  .title {
    flex: 0 0 auto;
    font-weight: 600;
  }

  .banner.warning .title {
    color: var(--status-modify);
  }

  .banner.error .title {
    color: var(--status-delete);
  }

  .detail {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    user-select: text;
  }

  button {
    flex: 0 0 auto;
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:disabled {
    opacity: 0.45;
  }

  button:not(:disabled):hover {
    border-color: var(--status-ref);
  }
</style>
