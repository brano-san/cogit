import { describe, expect, it } from "vitest";
import { confirmation } from "./confirm.svelte";

describe("a confirmation with a choice", () => {
  it("answers with the option picked, starting at the given one", async () => {
    const asked = confirmation.askWithChoice({
      title: "Run gc",
      message: "?",
      confirm: "Run",
      choice: { label: "Prune", value: "a", options: [["a", "A"], ["b", "B"]] },
    });
    expect(confirmation.open?.choice?.value).toBe("a");
    confirmation.answer(true, false, "b");
    expect(await asked).toEqual({ yes: true, choice: "b" });
  });

  it("answers with no choice when canceled", async () => {
    const asked = confirmation.askWithChoice({
      title: "Run gc",
      message: "?",
      confirm: "Run",
      choice: { label: "Prune", value: "a", options: [["a", "A"]] },
    });
    confirmation.answer(false);
    expect(await asked).toEqual({ yes: false, choice: null });
  });
});
