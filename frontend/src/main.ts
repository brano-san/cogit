import { mount } from "svelte";
import App from "./App.svelte";
import WindowFrame from "$components/layout/WindowFrame.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from index.html");
}

export default mount(WindowFrame, { target, props: { page: App, main: true } });
