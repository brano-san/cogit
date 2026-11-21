<script lang="ts">
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { backendLabels } from "$lib/ipc/repo-rows";
  import { sshHost } from "$lib/ssh-repositories";

  let { remote, title, onpick, onbrowse }: {
    remote: boolean;
    title: string;
    onpick: (path: string | null) => void;
    onbrowse: () => Promise<string | null>;
  } = $props();

  let address = $state("");
  let checked = $state("");
  let host = $state<string | null>(null);
  let problem = $state<string | null>(null);
  const path = $derived(address.trim());
  const ready = $derived(path !== "" && path === checked && problem === null && (!remote || host !== null));

  $effect(() => {
    const value = path;
    let active = true;
    checked = "";
    host = null;
    problem = null;
    if (!value) return;
    void backendLabels([value]).then(([label]) => {
      if (!active) return;
      if (label === "invalid SSH address") problem = "Enter a valid SSH address without a password.";
      host = sshHost(label);
      checked = value;
    }).catch((err) => {
      if (active) problem = String(err);
    });
    return () => { active = false; };
  });

  async function browse() {
    const picked = await onbrowse();
    if (picked) onpick(picked);
  }
</script>

<TemplateDialog
  {title}
  onclose={() => onpick(null)}
  status={problem ?? (remote && checked !== "" && checked === path && !host ? "Enter an SSH repository address." : null)}
  actions={[
    ...(!remote ? [{ label: "Browse…", onclick: () => void browse() }] : []),
    { label: host ? `Open on ${host} through SSH` : "Open", primary: true, disabled: !ready, onclick: () => onpick(path) },
  ]}
>
  <label for="repository-address">{remote ? "SSH repository address" : "Folder or SSH repository address"}</label>
  <input class="mono" id="repository-address" type="text" bind:value={address} spellcheck="false" data-autofocus
    placeholder="ssh://user@host/path or user@host:path" />
</TemplateDialog>
