<script lang="ts">
  import { initialsOf } from "$lib/avatars";
  import { avatars } from "$stores/avatars.svelte";

  /** One author's face, the same in the commit list and in the Diff panel. Nothing is
      drawn while avatars are off, so the column disappears rather than sitting empty. */
  interface Props {
    name: string;
    email: string;
    size?: number;
  }

  let { name, email, size = 16 }: Props = $props();

  const face = $derived(avatars.look(email));
  const letters = $derived(face?.initials || initialsOf(name, email));
</script>

{#if avatars.enabled}
  <span
    class="avatar"
    style:--avatar-size="{size}px"
    style:background={face?.image ? "transparent" : (face?.color ?? "var(--surface-raised)")}
    title={email}
  >
    {#if face?.image}
      <img src={face.image} alt="" width={size} height={size} />
    {:else}
      {letters}
    {/if}
  </span>
{/if}

<style>
  /* Fixed width so the author column does not shift as pictures arrive. */
  .avatar {
    flex: 0 0 var(--avatar-size);
    width: var(--avatar-size);
    height: var(--avatar-size);
    border-radius: var(--r-sm);
    overflow: hidden;
    color: var(--text-on-accent);
    font-size: calc(var(--avatar-size) * 0.56);
    font-weight: 600;
    line-height: var(--avatar-size);
    text-align: center;
  }

  .avatar img {
    display: block;
    width: var(--avatar-size);
    height: var(--avatar-size);
  }
</style>
