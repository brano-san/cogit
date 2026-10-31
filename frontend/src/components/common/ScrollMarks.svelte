<script lang="ts">
  /** Change marks over the right edge of a pane, where its scrollbar is: one per hunk, in
      the hunk's color; a click brings the hunk into view. */
  interface Props {
    marks: { id: number; top: number; height: number; tone: string | null }[];
    onpick: (id: number) => void;
  }

  let { marks, onpick }: Props = $props();
</script>

<div class="marks" aria-hidden="true">
  {#each marks as mark (mark.id)}
    <button
      type="button"
      tabindex="-1"
      title="Go to this change"
      class="mark {mark.tone ?? 'plain'}"
      style:top="{mark.top}px"
      style:height="{mark.height}px"
      onclick={() => onpick(mark.id)}
    ></button>
  {/each}
</div>

<style>
  .marks {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 3;
    width: 6px;
    pointer-events: none;
  }

  .mark {
    position: absolute;
    left: 0;
    width: 6px;
    padding: 0;
    border: 0;
    border-radius: 1px;
    background: var(--diff-connector-stroke);
    pointer-events: auto;
    cursor: pointer;
  }

  .mark.add {
    background: var(--diff-add-word);
  }

  .mark.del {
    background: var(--diff-del-word);
  }

  .mark.changed {
    background: var(--diff-changed-word);
  }

  .mark.auto {
    background: var(--c-stash-bg);
  }

  .mark.move {
    background: var(--diff-move-word);
  }

  .mark.conflict {
    background: var(--status-danger);
  }

  .mark.resolved {
    background: var(--status-add);
  }
</style>
