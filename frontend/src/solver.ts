import { mount } from "svelte";
import SolverWindow from "./SolverWindow.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from solver.html");
}

export default mount(SolverWindow, { target });
