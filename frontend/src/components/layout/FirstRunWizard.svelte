<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Radio from "$components/common/Radio.svelte";
  import { FIRST_RUN_STEPS, firstRunChoices, type FirstRunChoices } from "$lib/first-run";
  import { PERSPECTIVE_CHOICES } from "$lib/perspectives";
  import { THEMES, type Settings, type Theme } from "$lib/settings";
  import { themeStore } from "$stores/theme.svelte";

  interface Props {
    settings: Settings;
    onfinish: (choices: FirstRunChoices) => void;
  }

  let { settings, onfinish }: Props = $props();

  // svelte-ignore state_referenced_locally
  const before = firstRunChoices(settings);
  let choices = $state<FirstRunChoices>({ ...before });
  let at = $state(0);
  const step = $derived(FIRST_RUN_STEPS[at]);
  const last = $derived(at === FIRST_RUN_STEPS.length - 1);

  const PERSPECTIVE_HINTS: Record<string, string> = {
    main: "Every panel: repositories, branches, the graph, files, the diff and the commit message.",
    review: "Repositories, Commit Message and Worktrees hidden: the graph and the diff get the room.",
  };

  function pickTheme(theme: Theme) {
    choices.theme = theme;
    themeStore.apply(theme);
  }

  function next() {
    if (last) onfinish($state.snapshot(choices));
    else at += 1;
  }

  function skip() {
    themeStore.apply(before.theme);
    onfinish(before);
  }
</script>

<Dialog title="Welcome to Cogit" onclose={skip} onconfirm={next} width="min(520px, 92vw)">
  <div class="page">
    {#if step === "theme"}
      <h2>Choose a theme</h2>
      <p class="lead">Cogit switches as you pick, so you can see each one.</p>
      <div class="options" role="radiogroup" aria-label="Theme">
        {#each THEMES as [id, title] (id)}
          <Radio name="first-run-theme" checked={choices.theme === id} onchange={() => pickTheme(id)} label={title} />
        {/each}
      </div>
    {:else if step === "avatars"}
      <h2>Show author avatars?</h2>
      <p class="lead">
        Avatars come from Gravatar. It is asked for an MD5 of each author's email address, never the address itself.
      </p>
      <div class="options" role="radiogroup" aria-label="Author avatars">
        <Radio
          name="first-run-avatars"
          checked={choices.avatars === "gravatar"}
          onchange={() => (choices.avatars = "gravatar")}
          label="Show them, from Gravatar"
        />
        <Radio
          name="first-run-avatars"
          checked={choices.avatars === "off"}
          onchange={() => (choices.avatars = "off")}
          label="Do not show them"
        />
      </div>
    {:else if step === "updates"}
      <h2>Check for updates?</h2>
      <p class="lead">Once per start, quietly: Cogit speaks only when there is a new version.</p>
      <div class="options">
        <Checkbox
          checked={choices.autoUpdate}
          onchange={(checked) => (choices.autoUpdate = checked)}
          label="Check for updates when Cogit starts"
        />
      </div>
    {:else}
      <h2>Choose a perspective</h2>
      <p class="lead">The panels Cogit starts with. View ▸ Perspective: Main and Perspective: Review switch at any time.</p>
      <div class="options" role="radiogroup" aria-label="Perspective">
        {#each PERSPECTIVE_CHOICES as [id, title] (id)}
          <div class="choice">
            <Radio
              name="first-run-perspective"
              checked={choices.startupPerspective === id}
              onchange={() => (choices.startupPerspective = id)}
              label={title}
            />
            <span class="hint">{PERSPECTIVE_HINTS[id]}</span>
          </div>
        {/each}
      </div>
    {/if}
    <p class="later">Everything here can be changed later in Preferences.</p>
  </div>

  {#snippet footer()}
    <span class="count">Step {at + 1} of {FIRST_RUN_STEPS.length}</span>
    <span class="grow"></span>
    <button type="button" class="btn" onclick={skip}>Skip</button>
    <button type="button" class="btn" disabled={at === 0} onclick={() => (at -= 1)}>Back</button>
    <button type="button" class="btn primary" data-autofocus onclick={next}>{last ? "Finish" : "Next"}</button>
  {/snippet}
</Dialog>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-height: 200px;
  }

  h2 {
    margin: 0;
    color: var(--text-primary);
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .lead,
  .later {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .later {
    margin-top: auto;
    font-size: var(--fs-header);
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .choice {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .hint {
    padding-left: calc(var(--sp-6) + var(--sp-2));
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .count {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
