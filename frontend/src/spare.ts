import { mount, type Component } from "svelte";
import BlameWindow from "./BlameWindow.svelte";
import CommitWindow from "./CommitWindow.svelte";
import CompareWindow from "./CompareWindow.svelte";
import EditorWindow from "./EditorWindow.svelte";
import ErrorsWindow from "./ErrorsWindow.svelte";
import InvestigateWindow from "./InvestigateWindow.svelte";
import SolverWindow from "./SolverWindow.svelte";
import "./boot";
import { commands } from "$lib/ipc/bindings";
import { windowOf, type ChildWindow } from "$lib/spare-window";
import { bootTheme } from "$stores/theme.svelte";

const WINDOWS: Record<ChildWindow, Component> = {
  blame: BlameWindow,
  commit: CommitWindow,
  compare: CompareWindow,
  editor: EditorWindow,
  errors: ErrorsWindow,
  investigate: InvestigateWindow,
  solver: SolverWindow,
};

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from spare.html");
}

void (async () => {
  const address = await commands.spareWait();
  const kind = address === null ? null : windowOf(address);
  if (address === null || kind === null) return;
  history.replaceState(null, "", address);
  bootTheme();
  mount(WINDOWS[kind], { target });
})();
