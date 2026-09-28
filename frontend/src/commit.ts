import { mount } from "svelte";
import CommitWindow from "./CommitWindow.svelte";
import "./app.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from commit.html");
}

export default mount(CommitWindow, { target });
