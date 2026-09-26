<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Radio from "$components/common/Radio.svelte";
  import { shortOid } from "$lib/format";
  import type { Branch } from "$lib/ipc";
  import { branchFacts, pickProblem, type CheckoutChoice, type CheckoutOffer, type CheckoutPick } from "$lib/ref-checkout";

  /** The one Checkout dialog (item 40): the menu's Check Out and a double click in Branches
      or the graph. A local branch is only described; anything else offers its choices. */
  interface Props {
    offer: CheckoutOffer;
    branches: readonly Branch[];
    oncheckout: (pick: CheckoutPick, dontShowAgain: boolean) => void;
    onclose: () => void;
  }

  let { offer, branches, oncheckout, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let choice = $state<CheckoutChoice>(offer.initial);
  // svelte-ignore state_referenced_locally
  let name = $state(offer.create?.name ?? "");
  let track = $state(true);
  let dontShowAgain = $state(false);

  const pick = $derived<CheckoutPick>({ choice, name, track });
  const problem = $derived(pickProblem(offer, pick, branches));
  const typed = $derived(offer.create !== null && name.trim() !== offer.create.name);

  function submit() {
    if (problem === null) oncheckout(pick, dontShowAgain);
  }
</script>

<Dialog title="Check Out" {onclose} onconfirm={submit} dirty={typed} width="min(540px, 92vw)">
  <div class="form">
    {#if offer.plain}
      <p class="what">Check out the local branch <span class="mono">{offer.plain.name}</span>.</p>
      <p class="explanation">
        {branchFacts(offer.plain)} Local changes stay in the working tree where git can carry them
        over; where it cannot, Cogit offers to stash them first.
      </p>
      <Checkbox bind:checked={dontShowAgain} label="Don't show again" />
    {:else}
      <p class="what">Check out <span class="mono">{offer.source}</span></p>
      <fieldset>
        <legend class="caption">How</legend>
        {#if offer.create}
          <Radio name="checkout-choice" checked={choice === "create"} onchange={() => (choice = "create")}>
            <span class="label">Create local branch</span>
          </Radio>
          <div class="nested">
            <input
              type="text"
              bind:value={name}
              aria-label="Local branch name"
              aria-invalid={choice === "create" && problem !== null}
              aria-describedby={choice === "create" && problem ? "checkout-name-problem" : undefined}
              data-autofocus={offer.initial === "create" || undefined}
              onfocus={() => (choice = "create")}
            />
            {#if choice === "create" && problem}
              <span class="problem" id="checkout-name-problem">{problem}</span>
            {/if}
            {#if offer.create.remote}
              <Checkbox
                bind:checked={track}
                disabled={choice !== "create"}
                label="Track remote branch"
                title="Make {offer.create.remote} the new branch's upstream, for pull, push and ↑↓"
              />
            {/if}
          </div>
        {/if}
        {#if offer.detach}
          <Radio
            name="checkout-choice"
            checked={choice === "detach"}
            disabled={offer.detach.blocked !== null}
            onchange={() => (choice = "detach")}
          >
            <span class="text">
              <span class="label">Don't create a local branch (just read-only)</span>
              <span class="explanation">
                {offer.detach.blocked ??
                  `HEAD is detached at ${shortOid(offer.detach.oid)}: commits made there belong to no branch until you add one.`}
              </span>
            </span>
          </Radio>
        {/if}
        {#if offer.local}
          <Radio
            name="checkout-choice"
            checked={choice === "local"}
            disabled={offer.local.blocked !== null}
            onchange={() => (choice = "local")}
          >
            <span class="text">
              <span class="label">{offer.local.label}</span>
              <span class="explanation">{offer.local.blocked ?? offer.local.explanation}</span>
            </span>
          </Radio>
        {/if}
      </fieldset>
    {/if}
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button
      type="button"
      class="btn primary"
      data-autofocus={offer.initial !== "create" || undefined}
      disabled={problem !== null}
      title={problem ?? undefined}
      onclick={submit}>Checkout</button
    >
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    min-width: 0;
    font-size: var(--fs-dense);
  }

  .what {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .caption,
  .explanation {
    color: var(--text-secondary);
  }

  .explanation {
    margin: 0;
    line-height: 1.4;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0;
    margin-bottom: var(--sp-2);
  }

  .nested {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-left: calc(var(--checkbox-size) + var(--sp-3));
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .label {
    font-weight: 600;
  }

  .problem {
    color: var(--status-delete);
    font-size: 11px;
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
