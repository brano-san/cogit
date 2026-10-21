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
  /** The eye beside the field: the typed secret in plain text, until the next opening. */
  let shown = $state(false);

  // Each opening starts from what it was asked with.
  $effect.pre(() => {
    if (!request) return;
    host = request.host;
    username = request.username ?? "";
    secret = "";
    remember = true;
    shown = false;
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
    <div class="field">
      <label class="lbl" for="sign-in-secret">Password or token</label>
      <span class="secret">
        <input
          id="sign-in-secret"
          type={shown ? "text" : "password"}
          spellcheck="false"
          autocomplete="current-password"
          data-autofocus={!request.adding && username !== "" ? true : undefined}
          bind:value={secret}
        />
        <button
          type="button"
          class="btn sm eye"
          aria-pressed={shown}
          aria-label={shown ? "Hide password" : "Show password"}
          title={shown ? "Hide password" : "Show password"}
          onclick={() => (shown = !shown)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true"
            ><path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8Z" /><circle cx="8" cy="8" r="2" />{#if shown}<path
                d="M2.5 13.5 13.5 2.5"
              />{/if}</svg
          >
        </button>
      </span>
    </div>
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

  .secret {
    display: flex;
    gap: var(--sp-2);
  }

  .secret input {
    flex: 1 1 auto;
    min-width: 0;
  }

  .eye svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .error {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
