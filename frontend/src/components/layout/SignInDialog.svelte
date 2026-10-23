<script lang="ts">
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import { sshKeys, type SshKeys } from "$lib/ipc";
  import { keyFileProblem } from "$lib/sign-in";
  import { signIn } from "$stores/sign-in.svelte";

  /** Sign in: a remote refused, or Preferences ▸ Authentication ▸ Add… (F-082). Two ways in:
      a login with a password or token for HTTPS, an ssh private key for ssh (R-891). */
  const request = $derived(signIn.open);

  let tab = $state<"login" | "ssh">("login");
  let host = $state("");
  let username = $state("");
  let secret = $state("");
  let keyPath = $state("");
  let passphrase = $state("");
  let remember = $state(true);
  /** The eye beside a field: the typed secret in plain text, until the next opening. */
  let shown = $state(false);
  let keys = $state.raw<SshKeys>({ dir: "", keys: [] });

  // Each opening starts from what it was asked with.
  $effect.pre(() => {
    if (!request) return;
    tab = request.ssh ? "ssh" : "login";
    host = request.host;
    username = request.username ?? "";
    secret = "";
    keyPath = "";
    passphrase = "";
    remember = true;
    shown = false;
    sshKeys()
      .then((found) => {
        keys = found;
        if (keyPath === "" && found.keys.length > 0) keyPath = found.keys[0]!;
      })
      .catch((err: unknown) => console.error("could not look for ssh keys", err));
  });

  /** A refusal names its kind of remote: the other tab would not answer it. */
  const fixed = $derived(request !== null && !request.adding);
  const keyProblem = $derived(tab === "ssh" ? keyFileProblem(keyPath) : null);
  const ready = $derived(
    host.trim() !== "" && (tab === "login" ? secret !== "" : keyPath.trim() !== "" && keyProblem === null),
  );

  function submit() {
    if (!request || !ready) return;
    const name = username.trim();
    signIn.answer(
      tab === "login"
        ? { host: host.trim(), username: name === "" ? null : name, secret, keyPath: null, remember: request.adding || remember }
        : { host: host.trim(), username: null, secret: passphrase, keyPath: keyPath.trim(), remember: request.adding || remember },
    );
  }

  async function browse() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      title: "Select SSH Private Key",
      multiple: false,
      directory: false,
      defaultPath: keys.dir || undefined,
    });
    if (typeof picked === "string") keyPath = picked;
  }

  function fileName(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }
</script>

{#snippet eye()}
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
{/snippet}

{#if request}
  <TemplateDialog
    title={request.adding ? "Add Sign-in" : "Sign In"}
    onclose={() => signIn.answer(null)}
    status={keyProblem}
    actions={[{ label: request.adding ? "Add" : "Sign In", onclick: submit, primary: true, disabled: !ready }]}
  >
    {#if !request.adding}
      <p class="sub">
        {request.host}
        {request.ssh ? "did not take an ssh key. Choose the private key to sign in with." : "needs a user name and a password or access token."}
      </p>
    {/if}
    {#if request.error}
      <Callout kind="danger"><span class="error mono">{request.error}</span></Callout>
    {/if}
    <div class="tabs" role="tablist" aria-label="Sign-in kind">
      <button
        type="button"
        role="tab"
        aria-selected={tab === "login"}
        class:active={tab === "login"}
        disabled={fixed && request.ssh}
        onclick={() => (tab = "login")}>Login + Password/Token</button
      >
      <button
        type="button"
        role="tab"
        aria-selected={tab === "ssh"}
        class:active={tab === "ssh"}
        disabled={fixed && !request.ssh}
        onclick={() => (tab = "ssh")}>SSH Key</button
      >
    </div>
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
    {#if tab === "login"}
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
          {@render eye()}
        </span>
      </div>
    {:else}
      <div class="field">
        <label class="lbl" for="sign-in-key">Private key</label>
        <span class="secret">
          <input id="sign-in-key" type="text" class="mono" spellcheck="false" bind:value={keyPath} />
          <button type="button" class="btn sm eye" onclick={() => void browse()}>Files…</button>
        </span>
      </div>
      {#if keys.keys.length > 0}
        <div class="keys" role="radiogroup" aria-label="Keys found in {keys.dir}">
          {#each keys.keys as path (path)}
            <OptionRow
              kind="radio"
              name="sign-in-ssh-key"
              label={fileName(path)}
              tip={path}
              checked={keyPath === path}
              onchange={(on) => {
                if (on) keyPath = path;
              }}
            />
          {/each}
        </div>
      {:else}
        <p class="sub">No private key in {keys.dir || "~/.ssh"}: choose one with Files….</p>
      {/if}
      <div class="field">
        <label class="lbl" for="sign-in-passphrase">Passphrase (if the key has one)</label>
        <span class="secret">
          <input
            id="sign-in-passphrase"
            type={shown ? "text" : "password"}
            spellcheck="false"
            autocomplete="off"
            data-autofocus={!request.adding ? true : undefined}
            bind:value={passphrase}
          />
          {@render eye()}
        </span>
      </div>
    {/if}
    {#if !request.adding}
      <OptionRow bind:checked={remember} label="Remember" hint="Kept in Cogit's password store for this host" />
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

  .tabs {
    display: flex;
    align-self: flex-start;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    overflow: hidden;
  }

  .tabs button {
    border: 0;
    border-radius: 0;
  }

  .tabs button + button {
    border-left: 1px solid var(--field-border);
  }

  .tabs button.active {
    background: var(--state-selected);
    color: var(--text-primary);
  }

  .keys {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .secret {
    display: flex;
    gap: var(--sp-2);
  }

  /* As tall as the field beside it, not the small button height: no step between them. */
  .secret .eye {
    align-self: stretch;
    height: auto;
    padding-block: 0;
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
