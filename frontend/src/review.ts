import { mount } from "svelte";
import ReviewWindow from "./ReviewWindow.svelte";
import "./boot";

const target = document.getElementById("app");
if (!target) {
  throw new Error("mount target #app is missing from review.html");
}

export default mount(ReviewWindow, { target });
