import { mount } from "svelte";
import MergeWindow from "./MergeWindow.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from merge.html");
}

export default mount(MergeWindow, { target });
