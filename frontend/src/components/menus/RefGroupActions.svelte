<script lang="ts">
  import RemotePropertiesDialog from "./RemotePropertiesDialog.svelte";
  import { createBranch, popupContextMenu, type ContextItem, type RepoId } from "$lib/ipc";
  import {
    fetchDepth,
    fetchMore,
    remoteInfo,
    removeRemote,
    renameRemote,
    setRemoteProperties,
    type RemoteInfo,
  } from "$lib/ipc/remotes";
  import { shortOid } from "$lib/format";
  import { branchNameProblem } from "$lib/names";
  import { splitUpstream } from "$lib/push-to";
  import {
    GROUP_MENU_PREFIX,
    claimsNode,
    depthProblem,
    groupMenu,
    remoteMenu,
    remoteNameProblem,
    remotePull,
    type RemoteFacts,
  } from "$lib/ref-group-menus";
  import { buildRefTree, tickStates, toggleNode, type RefNode, type RefTreeInput } from "$lib/ref-nodes";
  import { commit } from "$stores/commit.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { errors } from "$stores/errors.svelte";
  import { network } from "$stores/network.svelte";
  import { notices } from "$stores/notices.svelte";
  import { prompt } from "$stores/prompt.svelte";
  import { refDialogs } from "$stores/ref-dialogs.svelte";
  import { refs } from "$stores/refs.svelte";
  import { remoteDialogs } from "$stores/remote-dialogs.svelte";
  import { repoPulse } from "$stores/repo-pulse.svelte";
  import { repository } from "$stores/repository.svelte";
  import { settings } from "$stores/settings.svelte";

  /** The Branches menus of HEAD, the headings, the folders and a remote's heading (#19 of
      25.09); the refs themselves keep `RefActions`. App hands over the right-click and the
      chosen item id, and lends its refreshes. */
  interface Props {
    /** What the tree in Branches is built from: Toggle counts the same rows its box does. */
    input: RefTreeInput;
    afterRefChange: (worked?: RepoId) => Promise<void>;
    afterFetch: (worked: RepoId) => Promise<void>;
    reloadGraph: () => Promise<void>;
    addTag: () => void;
  }

  let { input, afterRefChange, afterFetch, reloadGraph, addTag }: Props = $props();

  interface Target {
    node: RefNode;
    remote: RemoteFacts | null;
    info: RemoteInfo | null;
  }

  /** The chosen id comes back as a `menu-command` event, after the popup has closed. */
  let target: Target | null = null;
  let asked = 0;

  const tree = $derived(buildRefTree(input));

  export function claims(node: RefNode): boolean {
    return claimsNode(node);
  }

  function toggleBlocked(node: RefNode): string | null {
    if (tickStates(tree, refs.visible).get(node.id)?.tickable !== false) return null;
    return node.remote !== undefined ? "nothing fetched from it yet" : "nothing here is drawn in the graph";
  }

  export async function context(node: RefNode, x: number, y: number) {
    const id = repository.current?.repo;
    if (!id || !claims(node)) return;
    const token = ++asked;
    let items: ContextItem[];
    let remote: RemoteFacts | null = null;
    let info: RemoteInfo | null = null;
    if (node.remote === undefined) {
      items = groupMenu(node, toggleBlocked(node));
    } else {
      const configured = network.remotes.includes(node.remote);
      info = configured
        ? await remoteInfo(id, node.remote).catch((err: unknown) => {
            errors.report(err, "Could not read the remote");
            return null;
          })
        : null;
      if (token !== asked) return;
      remote = remoteFacts(node.remote, configured, info, toggleBlocked(node));
      items = remoteMenu(remote);
    }
    target = { node, remote, info };
    await popupContextMenu(items, x, y).catch(() => {});
  }

  function remoteFacts(name: string, configured: boolean, info: RemoteInfo | null, toggle: string | null): RemoteFacts {
    const head = repository.current?.head;
    const branch = repository.localBranches.find((entry) => entry.isHead);
    const upstream = branch?.upstream ?? null;
    return {
      remote: name,
      configured,
      head: head?.kind === "branch" ? { name: head.name, upstream } : null,
      upstreamRemote: upstream ? (splitUpstream(upstream, network.remotes)?.remote ?? null) : null,
      url: info?.url ?? null,
      shallow: info?.shallow ?? false,
      toggle,
    };
  }

  /** True when the id came from one of these menus and has been taken care of. */
  export function run(id: string): boolean {
    if (!id.startsWith(GROUP_MENU_PREFIX)) return false;
    void act(id.slice(GROUP_MENU_PREFIX.length));
    return true;
  }

  /** A box's click, from the menu: the three-state rule of the tree (R-158). */
  export async function toggle(node: RefNode) {
    refs.set(toggleNode(tree, node.id, refs.visible));
    await reloadGraph();
  }

  async function attempt(title: string, step: () => Promise<unknown>): Promise<boolean> {
    try {
      await step();
      return true;
    } catch (err) {
      errors.report(err, title);
      return false;
    }
  }

  async function act(name: string) {
    const id = repository.current?.repo;
    const at = target;
    if (!id || !at) return;
    const facts = at.remote;
    switch (name) {
      case "toggle":
        return toggle(at.node);
      case "add-branch":
        return addBranch(id);
      case "add-tag":
        return addTag();
    }
    if (!facts) return;
    const remote = facts.remote;
    switch (name) {
      case "push-to":
        if (facts.head) refDialogs.push = { kind: "branch", name: facts.head.name, upstream: facts.head.upstream, remote };
        return;
      case "pull":
        return remotePull(facts) === "pull" ? pull(id, remote) : fetch(id, facts);
      case "fetch":
        return fetch(id, facts);
      case "fetch-more":
        return fetchMoreFrom(id, remote);
      case "rename":
        return rename(id, remote);
      case "delete":
        return remove(id, remote, at.info?.url ?? null);
      case "copy-url":
        return copy(at.info?.url ?? "");
      case "set-depth":
        return setDepth(id, remote);
      case "properties":
        if (at.info) remoteDialogs.properties = at.info;
        return;
    }
  }

  async function addBranch(id: RepoId) {
    const head = repository.current?.head;
    const oid = commit.oid ?? (head && head.kind !== "unborn" ? head.oid : null);
    if (!oid) {
      errors.message("There is no commit to start a branch from yet.", "Could not add the branch");
      return;
    }
    const name = await prompt.ask({
      title: `Add Branch at ${shortOid(oid)}`,
      label: "Name",
      confirm: "Add Branch",
      validate: (value) => branchNameProblem(value, repository.localBranches.map((entry) => entry.name)),
    });
    if (name === null) return;
    await attempt("Could not add the branch", () => createBranch(id, name, oid, false));
    await afterRefChange(id);
  }

  async function pull(id: RepoId, remote: string) {
    const root = repository.current?.root;
    const done = await attempt("Could not pull", () =>
      network.pull(id, remote, settings.current.pullMode === "ffOnly"),
    );
    if (done && root) repoPulse.fetched(root);
    await afterRefChange(id);
  }

  async function fetch(id: RepoId, facts: RemoteFacts) {
    const root = repository.current?.root;
    const done = await attempt(`Could not fetch ${facts.remote}`, () => network.fetch(id, facts.remote));
    if (done && root && facts.remote === facts.upstreamRemote) repoPulse.fetched(root);
    await afterFetch(id);
  }

  async function fetchMoreFrom(id: RepoId, remote: string) {
    let changed = true;
    const done = await attempt(`Could not fetch more from ${remote}`, () =>
      network.run(id, "Fetching", async (onLine) => {
        changed = await fetchMore(id, remote, onLine);
      }),
    );
    if (done && !changed) {
      notices.inform("Nothing new", `Every branch and tag of ${remote} is here already.`);
    }
    await afterFetch(id);
  }

  async function setDepth(id: RepoId, remote: string) {
    const depth = await prompt.ask({
      title: `Set Depth of ${remote}`,
      label: "Depth in commits, from each branch tip",
      confirm: "Fetch",
      validate: depthProblem,
    });
    if (depth === null) return;
    await attempt(`Could not fetch ${remote} at that depth`, () =>
      network.run(id, "Fetching", (onLine) => fetchDepth(id, remote, Number(depth.trim()), onLine)),
    );
    await afterFetch(id);
  }

  async function rename(id: RepoId, remote: string) {
    const to = await prompt.ask({
      title: `Rename ${remote}`,
      label: "New name",
      value: remote,
      confirm: "Rename",
      validate: (value) => remoteNameProblem(value, network.remotes, remote),
    });
    if (to === null || to.trim() === remote) return;
    const name = to.trim();
    if (await attempt("Could not rename the remote", () => renameRemote(id, remote, name))) {
      refs.renameRemote(remote, name);
    }
    void refs.loadUrls(id);
    await afterRefChange(id);
  }

  async function remove(id: RepoId, remote: string, url: string | null) {
    const go = await confirmation.ask({
      title: "Delete Remote",
      message:
        `Delete the remote ${remote}${url ? ` (${url})` : ""}? Its remote branches go with it, and ` +
        "branches that track it stop tracking. Nothing changes on the server, and Undo cannot bring the remote back.",
      confirm: "Delete",
      warning: true,
    });
    if (!go) return;
    await attempt("Could not delete the remote", () => removeRemote(id, remote));
    void refs.loadUrls(id);
    await afterRefChange(id);
  }

  async function copy(text: string) {
    if (text === "") return;
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text).catch((err: unknown) => errors.report(err, "Could not copy"));
  }

  async function saveProperties(url: string, backgroundFetch: boolean) {
    const id = repository.current?.repo;
    const info = remoteDialogs.properties;
    remoteDialogs.properties = null;
    if (!id || !info) return;
    await attempt("Could not save the remote", () => setRemoteProperties(id, info.name, url, backgroundFetch));
    void refs.loadUrls(id);
    await afterRefChange(id);
  }
</script>

{#if remoteDialogs.properties}
  <RemotePropertiesDialog
    info={remoteDialogs.properties}
    everyMinutes={settings.current.backgroundFetchMinutes}
    onsave={(url, backgroundFetch) => void saveProperties(url, backgroundFetch)}
    onclose={() => (remoteDialogs.properties = null)}
  />
{/if}
