<script lang="ts">
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { signIn } from "$stores/sign-in.svelte";

  /** Sign in: a remote refused, or Preferences ▸ Authentication ▸ Add… (F-082). */
  const request = $derived(signIn.open);

  let host = $state("");
  let username = $state("");
  let secret = $state("");
  let remember = $state(true);

  // Each opening starts from what it was asked with.
  $effect.pre(() => {
    if (!request) return;
    host = request.host;
    username = request.username ?? "";
    secret = "";
    remember = true;
  });

  const ready = $derived(host.trim() !== "" && secret !== "");

  function submit() {
    if (!request || !ready) return;
    const name = username.trim();
    signIn.answer({
      host: host.trim(),
      username: name === "" ? null : name,
      secret,
      remember: request.adding || remember,
    });
  }
</script>

{#if request}
  <TemplateDialog
    title={request.adding ? "Add Sign-in" : "Sign In"}
    onclose={() => signIn.answer(null)}
    actions={[{ label: request.adding ? "Add" : "Sign In", onclick: submit, primary: true, disabled: !ready }]}
  >
    {#if !request.adding}
      <p class="sub">{request.host} needs a user name and a password or access token.</p>
    {/if}
    {#if request.error}
      <Callout kind="danger"><span class="error mono">{request.error}</span></Callout>
    {/if}
    <label class="field">
      <span class="lbl">Host</span>
      <input
        type="text"
        class="mono"
        spellcheck="false"
        readonly={!request.adding}
        placeholder="git.example.com or https://git.example.com/team/app.git"
        data-autofocus={request.adding ? true : undefined}
        bind:value={host}
      />
    </label>
    <label class="field">
      <span class="lbl">Username</span>
      <input
        type="text"
        spellcheck="false"
        autocomplete="username"
        placeholder="Leave empty for an access token"
        data-autofocus={!request.adding && username === "" ? true : undefined}
        bind:value={username}
      />
    </label>
    <label class="field">
      <span class="lbl">Password or token</span>
      <input
        type="password"
        autocomplete="current-password"
        data-autofocus={!request.adding && username !== "" ? true : undefined}
        bind:value={secret}
      />
    </label>
    {#if !request.adding}
      <OptionRow
        bind:checked={remember}
        label="Remember"
        hint={signIn.persistent
          ? "Kept in the system keychain for this host"
          : "No system keychain: kept until Cogit exits"}
      />
    {:else if !signIn.persistent}
      <p class="sub">No system keychain: kept until Cogit exits.</p>
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

  .error {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
