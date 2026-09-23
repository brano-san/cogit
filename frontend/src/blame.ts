import { mount } from "svelte";
import BlameWindow from "./BlameWindow.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from blame.html");
}

export default mount(BlameWindow, { target });
