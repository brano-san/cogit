<script lang="ts">
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
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
        ><path d="M3 8.5 6.5 12 13 4.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg
      >
      {successToast.text}
    </div>
  {/key}
{/if}

<style>
  .success {
    position: fixed;
    left: 50%;
    top: 50%;
    z-index: 60;
    transform: translate(-50%, -50%);
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

  .success svg {
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
