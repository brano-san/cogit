import { mount } from "svelte";
import InvestigateWindow from "./InvestigateWindow.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from investigate.html");
}

export default mount(InvestigateWindow, { target });
