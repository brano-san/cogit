import { mount } from "svelte";
import SolverWindow from "./SolverWindow.svelte";
import WindowFrame from "$components/layout/WindowFrame.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from solver.html");
}

export default mount(WindowFrame, { target, props: { page: SolverWindow } });
