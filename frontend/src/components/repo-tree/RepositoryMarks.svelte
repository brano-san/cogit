<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import FileStateIcon from "$components/common/FileStateIcon.svelte";
  import KindIcon from "$components/common/KindIcon.svelte";
  import { canPull, syncTooltip, type RowSync } from "$lib/repo-sync";

  let {
    sync,
    kind = "repository",
    gutter = true,
    tooltip = true,
  }: { sync: RowSync; kind?: "repository" | "submodule"; gutter?: boolean; tooltip?: boolean } = $props();
</script>

<span class="repo-icon" class:gutter>
  <KindIcon {kind} title={tooltip ? syncTooltip(sync) || undefined : null} changed={sync.dirty === true} />
  {#if sync.conflicted}<FileStateIcon state="conflicted" overlayOnly label="Unresolved merge conflicts" />{/if}
  {#if sync.ahead > 0}
    <Icon name="ahead" class="arrow push" label="Commits to push" />
  {/if}
  {#if sync.unknown}
    <span class="arrow unknown" role="img" aria-label="Unknown whether there is anything to pull"
      >?</span
    >
  {:else if canPull(sync)}
    <Icon name="behind" class="arrow pull" label="Commits to pull" />
  {/if}
</span>

<style>
  .repo-icon {
    --mark-halo: inherit;
    --kind-icon: var(--panel-icon);
    --file-icon: 20px;
    --sync-arrow: 12px;
    position: relative;
    display: inline-flex;
    flex: none;
  }

  .gutter {
    margin-inline: calc(var(--sp-3) / 2);
  }

  .repo-icon :global(.icon) {
    shape-rendering: geometricPrecision;
  }

  .repo-icon :global(.arrow) {
    position: absolute;
    width: var(--sync-arrow);
    height: var(--sync-arrow);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    filter: drop-shadow(0 0 1px var(--mark-halo, var(--surface-panel))) drop-shadow(0 0 1px var(--mark-halo, var(--surface-panel)));
  }

  .repo-icon :global(.arrow.push) {
    top: -3px;
    right: -5px;
    color: var(--indicator-push);
  }

  .repo-icon :global(.arrow.pull) {
    top: -3px;
    left: -5px;
    color: var(--indicator-pull);
  }

  .repo-icon :global(.arrow.unknown) {
    top: -4px;
    left: -4px;
    width: auto;
    height: auto;
    color: var(--indicator-unknown);
    font-size: var(--sync-arrow);
    font-weight: 700;
    line-height: 1;
  }

</style>
