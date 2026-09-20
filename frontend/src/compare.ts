import { mount } from "svelte";
import CompareWindow from "./CompareWindow.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from compare.html");
}

export default mount(CompareWindow, { target });
