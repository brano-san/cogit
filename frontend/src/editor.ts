import { mount } from "svelte";
import EditorWindow from "./EditorWindow.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from editor.html");
}

export default mount(EditorWindow, { target });
