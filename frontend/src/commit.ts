import { mount } from "svelte";
import CommitWindow from "./CommitWindow.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from commit.html");
}

export default mount(CommitWindow, { target });
