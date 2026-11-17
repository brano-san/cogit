<script lang="ts">
  import Callout from "$components/common/template/Callout.svelte";
  import ObjectCard, { type CardRow } from "$components/common/template/ObjectCard.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import type { DialogAction } from "$lib/dialog-template";
  import { hostKeys } from "$stores/host-keys.svelte";

  $effect(() => hostKeys.listen());

  const ask = $derived(hostKeys.current);

  const rows = $derived<CardRow[]>(
    ask
      ? [
          { label: "Host", value: ask.question.host, mono: true },
          ...(ask.question.address ? [{ label: "Address", value: ask.question.address, mono: true }] : []),
          { label: "Key type", value: ask.question.keyType, mono: true },
          { label: "Fingerprint", value: ask.question.fingerprint, mono: true },
        ]
      : [],
  );

  const actions = $derived<DialogAction[]>(
    ask
      ? [
          {
            label: "Trust and Connect",
            tip: "ssh adds the key to known_hosts and connects; it does not ask about this host again",
            primary: true,
            onclick: () => hostKeys.answer(ask.id, true),
          },
        ]
      : [],
  );
</script>

{#if ask}
  {#key ask.id}
    <TemplateDialog title="Unknown SSH Host Key" onclose={() => hostKeys.answer(ask.id, false)} {actions} destructive>
      <p class="sub">
        This server's SSH key is not in your known_hosts file. Compare the fingerprint with the one the server's owner
        publishes before you trust it: a key you do not recognize can mean the connection is being intercepted.
      </p>
      <ObjectCard {rows} />
      {#if ask.question.otherTypesKnown}
        <Callout kind="warning">
          A key of another type is already known for this host. Trust this one only if the server's owner confirms it.
        </Callout>
      {/if}
    </TemplateDialog>
  {/key}
{/if}
