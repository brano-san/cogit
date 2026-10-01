<script lang="ts">
  import ObjectCard, { type CardRow } from "$components/common/template/ObjectCard.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { parseStashMessage } from "$lib/dialog-template";
  import { dateTooltip, shortOid } from "$lib/format";
  import { stashes } from "$stores/stashes.svelte";

  /** Apply Stash (item 40): a double click on a stash and the menu's Apply Stash. */
  interface Props {
    index: number;
    message: string;
    onapply: (drop: boolean, restoreIndex: boolean) => void;
    onclose: () => void;
  }

  let { index, message, onapply, onclose }: Props = $props();

  let restoreIndex = $state(false);

  const rows = $derived.by(() => {
    const entry = stashes.entries.find((stash) => stash.index === index);
    const parsed = parseStashMessage(message);
    const list: CardRow[] = [{ label: "Stash", value: `stash@{${index}}`, mono: true }];
    if (parsed.branch) list.push({ label: "Branch", value: parsed.branch, mono: true });
    if (parsed.message) list.push({ label: "Message", value: parsed.message, clamp: 3 });
    if (entry) {
      list.push({ label: "Commit", value: shortOid(entry.oid), mono: true });
      list.push({ label: "Date", value: dateTooltip(entry.timestamp, -new Date().getTimezoneOffset()), mono: true });
    }
    return list;
  });
</script>

<TemplateDialog
  title={`Apply stash@{${index}}`}
  {onclose}
  actions={[
    { label: "Apply", onclick: () => onapply(false, restoreIndex) },
    {
      label: "Apply & Drop",
      primary: true,
      tip: "Removes the stash if it applied without conflicts",
      onclick: () => onapply(true, restoreIndex),
    },
  ]}
>
  <p class="sub">Changes will be applied to the working tree</p>
  <ObjectCard {rows} />
  <OptionRow bind:checked={restoreIndex} label="Restore Index" hint="Also restore what was staged (--index)" />
</TemplateDialog>
