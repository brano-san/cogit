import { mount } from "svelte";
import ErrorsWindow from "./ErrorsWindow.svelte";
import WindowFrame from "$components/layout/WindowFrame.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from errors.html");
}

export default mount(WindowFrame, { target, props: { page: ErrorsWindow } });
