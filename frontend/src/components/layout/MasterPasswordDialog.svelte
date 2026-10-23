<script lang="ts">
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import type { DialogAction } from "$lib/dialog-template";
  import { masterPassword } from "$stores/master-password.svelte";

  /** Cogit's password store asks for its master password, or for a new one (R-891). */
  const request = $derived(masterPassword.open);

  let current = $state("");
  let password = $state("");
  let confirm = $state("");
  let none = $state(false);

  $effect.pre(() => {
    if (!request) return;
    current = "";
    password = "";
    confirm = "";
    none = false;
  });

  const unlocking = $derived(request?.mode === "unlock");
  const asksCurrent = $derived(unlocking || (request?.mode === "change" && request.hasCurrent));
  const setsNew = $derived(!unlocking);
  const mismatch = $derived(setsNew && !none && confirm !== "" && password !== confirm);
  const ready = $derived(
    (!asksCurrent || current !== "") && (!setsNew || none || (password !== "" && password === confirm)),
  );

  const title = $derived(
    {
      unlock: "Master Password",
      create: "Set Master Password",
      change: "Change Master Password",
      forgot: "Forgot Master Password",
    }[request?.mode ?? "unlock"],
  );
  const confirmLabel = $derived(
    { unlock: "Unlock", create: "Set", change: "Change", forgot: "Delete and Set" }[request?.mode ?? "unlock"],
  );

  function submit() {
    if (!ready) return;
    masterPassword.answer({ current, password: none ? null : password });
  }

  const actions = $derived<DialogAction[]>([
    ...(asksCurrent
      ? [{ label: "Forgot…", tip: "Set a new master password; every stored sign-in is deleted", onclick: () => masterPassword.answer("forgot") }]
      : []),
    { label: confirmLabel, onclick: submit, primary: true, disabled: !ready },
  ]);
</script>

{#snippet secret(label: string, autofocus: boolean, value: string, set: (text: string) => void, disabled = false)}
  <label class="field">
    <span class="lbl">{label}</span>
    <input
      type="password"
      spellcheck="false"
      autocomplete="off"
      {disabled}
      data-autofocus={autofocus ? true : undefined}
      {value}
      oninput={(event) => set(event.currentTarget.value)}
    />
  </label>
{/snippet}

{#if request}
  <TemplateDialog
    {title}
    onclose={() => masterPassword.answer(null)}
    {actions}
    destructive={request.mode === "forgot"}
    status={mismatch ? "The passwords do not match." : null}
  >
    {#if request.mode === "unlock"}
      <p class="sub">Cogit's password store is locked. The master password opens it until Cogit exits.</p>
    {:else if request.mode === "create"}
      <p class="sub">
        Cogit keeps passwords, tokens and key passphrases in its own encrypted file. A master password protects it and is
        asked once per session.
      </p>
    {:else if request.mode === "forgot"}
      <Callout kind="danger">Every stored password, token and key passphrase is deleted.</Callout>
    {/if}
    {#if request.error}
      <Callout kind="danger">{request.error}</Callout>
    {/if}
    {#if asksCurrent}
      {@render secret(unlocking ? "Master password" : "Current master password", true, current, (t) => (current = t))}
    {/if}
    {#if setsNew}
      {@render secret("New master password", !asksCurrent, password, (t) => (password = t), none)}
      {@render secret("Confirm", false, confirm, (t) => (confirm = t), none)}
      <OptionRow
        bind:checked={none}
        label="Don't use a master password"
        hint="The file stays encrypted, but anyone who can read your files can open it"
      />
    {/if}
  </TemplateDialog>
{/if}

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    font-size: var(--fs-dense);
  }

  .lbl {
    color: var(--text-secondary);
  }
</style>
