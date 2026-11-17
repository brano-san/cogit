<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import { successToast, TOAST_FADE_MS, TOAST_HOLD_MS } from "$stores/success-toast.svelte";
</script>

{#if successToast.text}
  {#key successToast.id}
    <div
      class="success"
      role="status"
      style:--hold="{TOAST_HOLD_MS}ms"
      style:--fade="{TOAST_FADE_MS}ms"
    >
      <Icon name="check" />
      {successToast.text}
    </div>
  {/key}
{/if}

<style>
  .success {
    --icon-size: 14px;
    position: fixed;
    z-index: 60;
    inset: 0;
    margin: auto;
    width: fit-content;
    height: fit-content;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-3) var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--status-add);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    font-size: var(--fs-dense);
    pointer-events: none;
    animation: fade var(--fade) ease-in var(--hold) forwards;
  }

  .success :global(svg) {
    color: var(--status-add);
  }

  @keyframes fade {
    to {
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .success {
      animation: none;
    }
  }
</style>
