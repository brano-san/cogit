import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = {
  commandOutcome: vi.fn(),
  repositoryHealth: vi.fn(),
  readSettings: vi.fn(),
  writeSetting: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { CogitError } = await import("$lib/ipc");
const { notices } = await import("./notices.svelte");
const { errors } = await import("./errors.svelte");
const { health } = await import("./health.svelte");

const refusal = (text: string) => new CogitError({ kind: "invalidState", data: text });

const run = (id: number, summary = "error: failed to push some refs") => ({
  id,
  repo: "C:/repos/cogit",
  command: "git push origin master",
  exitCode: 1,
  stdout: "",
  stderr: `${summary}\nhint: Updates were rejected`,
  durationMs: 12,
  operation: "Push",
  severity: "failure" as const,
  summary,
  startedAtMs: 1,
});

const failed = (id: number, summary?: string) => ({
  id,
  repo: "C:/repos/cogit",
  operation: "Push",
  severity: "failure" as const,
  summary: summary ?? "error: failed to push some refs",
});

const ignoreCase = { kind: "ignoreCaseMismatch", configured: false, actual: true } as const;
const goneWorktree = {
  kind: "danglingWorktree",
  name: "old",
  target: "E:/gone",
  foreign: false,
} as const;

async function warn(...issues: object[]) {
  commands.repositoryHealth.mockResolvedValue({
    status: "ok",
    data: issues.map((issue) => ({ module: "", issue })),
  });
  await health.check(1 as never, "C:/repos/cogit", "cogit");
}

describe("the notification window", () => {
  beforeEach(async () => {
    notices.dismissAll();
    commands.commandOutcome.mockReset();
    commands.commandOutcome.mockImplementation((id: number) => Promise.resolve(run(id)));
    commands.writeSetting.mockResolvedValue({ status: "ok", data: null });
    commands.readSettings.mockResolvedValue("{}");
    await warn();
  });

  it("has nothing to say until something goes wrong", () => {
    expect(notices.current).toBeUndefined();
    expect(notices.errorCount).toBe(0);
  });

  it("names what did not work, not that Cogit stopped", () => {
    errors.report(refusal("Not a Git repository"), "Could not open the repository");

    expect(notices.current?.title).toBe("Could not open the repository");
    expect(notices.current?.body).toContain("Not a Git repository");
    expect(notices.current?.severity).toBe("error");
  });

  it("does not repeat itself when the same failure is reported again", () => {
    errors.report(refusal("Not a Git repository"), "Could not open the repository");
    errors.report(refusal("Not a Git repository"), "Could not open the repository");

    expect(notices.all).toHaveLength(1);
  });

  it("ignores a null, which is what a store with no error hands over", () => {
    errors.report(null, "Could not show the diff");
    expect(notices.all).toHaveLength(0);
  });

  it("takes whatever a catch hands over, not only a CogitError", () => {
    errors.report({ kind: "internal", data: "C:/x.log" }, "Could not reveal the log");
    errors.report(new Error("window blocked"), "Could not open the window");
    errors.report({ message: "This repository has no remote." }, "Could not fetch");

    expect(notices.all.map((notice) => notice.body)).toEqual([
      "Invalid repository state: This repository has no remote.",
      "Invalid repository state: window blocked",
      "Internal error: C:/x.log",
    ]);
  });

  it("puts the newest error in front and moves to the next when one is closed", () => {
    errors.report(refusal("first"), "Could not merge");
    errors.report(refusal("second"), "Could not rebase");
    expect(notices.current?.body).toContain("second");

    notices.dismiss();
    expect(notices.current?.body).toContain("first");
  });

  it("closes after the last entry and clears the footer's error", () => {
    errors.report(refusal("first"), "Could not merge");
    errors.report(refusal("second"), "Could not rebase");

    notices.dismiss();
    notices.dismiss();

    expect(notices.current).toBeUndefined();
    expect(notices.errorCount).toBe(0);
  });

  it("puts every error ahead of every warning", async () => {
    await warn(goneWorktree, ignoreCase);
    errors.report(refusal("first"), "Could not merge");
    errors.report(refusal("second"), "Could not rebase");

    expect(notices.all.map((notice) => notice.severity)).toEqual([
      "error",
      "error",
      "warning",
      "warning",
    ]);
    expect(notices.all[0]?.body).toContain("second");
  });

  it("switches to an error that arrives while a warning is open, and keeps the warning", async () => {
    await warn(goneWorktree, ignoreCase);
    notices.step(1);
    expect(notices.current?.title).toContain("core.ignoreCase");

    errors.report(refusal("broken"), "Could not show the diff");
    expect(notices.current?.title).toBe("Could not show the diff");
    expect(notices.errorCount).toBe(1);

    notices.dismiss();
    expect(notices.all.map((notice) => notice.severity)).toEqual(["warning", "warning"]);
    expect(notices.errorCount).toBe(0);
  });

  it("walks through the queue without walking off either end", async () => {
    await warn(goneWorktree);
    errors.report(refusal("broken"), "Could not merge");

    notices.step(-1);
    expect(notices.at).toBe(0);
    notices.step(1);
    expect(notices.current?.severity).toBe("warning");
    notices.step(1);
    expect(notices.at).toBe(1);
  });

  it("reminds later about a warning closed with the cross, until the next open", async () => {
    await warn(goneWorktree, ignoreCase);

    notices.dismiss();
    expect(notices.all).toHaveLength(1);
    expect(notices.current?.title).toContain("core.ignoreCase");

    await warn(goneWorktree, ignoreCase);
    expect(notices.all).toHaveLength(2);
  });

  it("forgets an ignored warning for good", async () => {
    await warn(ignoreCase);
    await notices.ignore();

    expect(notices.all).toHaveLength(0);
    await warn(ignoreCase);
    expect(notices.all).toHaveLength(0);
    await health.unignore("C:/repos/cogit", "ignoreCaseMismatch:false");
  });
});

describe("a failed git command in the notification window", () => {
  beforeEach(async () => {
    notices.dismissAll();
    commands.commandOutcome.mockReset();
    commands.commandOutcome.mockImplementation((id: number) => Promise.resolve(run(id)));
    await warn();
  });

  it("is titled by the operation, with its output and a way to the output window", async () => {
    await notices.command(failed(7));

    expect(notices.current?.title).toBe("Push failed");
    expect(notices.current?.output).toContain("hint: Updates were rejected");
    expect(notices.current?.record).toBe(7);
  });

  it("copies the command and the exit code with the output, like the output window", async () => {
    await notices.command(failed(7));

    const report = notices.current?.report ?? "";
    expect(report).toContain("Command: git push origin master");
    expect(report).toContain("Exit code: 1");
    expect(report).toContain("hint: Updates were rejected");
  });

  it("counts the event and the rejected call as one failure", async () => {
    const rejected = new CogitError({ kind: "command", data: run(7) });
    await Promise.all([notices.command(failed(7)), errors.report(rejected, "Could not push")]);

    expect(notices.all).toHaveLength(1);
  });

  it("counts a repeat instead of queueing it again", async () => {
    await notices.command(failed(7));
    await notices.command(failed(8));

    expect(notices.all).toHaveLength(1);
    expect(notices.current?.repeats).toBe(2);
    expect(notices.current?.record).toBe(8);
  });

  it("keeps a failure whose record already rotated out of the journal", async () => {
    commands.commandOutcome.mockResolvedValue(null);
    await notices.command(failed(7));

    expect(notices.current?.title).toBe("Push failed");
    expect(notices.current?.body).toContain("failed to push");
  });

  it("points to the output window instead of holding a hook's thousand lines", async () => {
    const long = Array.from({ length: 2000 }, (_, i) => `line ${i}`).join("\n");
    commands.commandOutcome.mockResolvedValue({ ...run(7), stderr: long });
    await notices.command(failed(7));

    expect(notices.current?.output).toBeUndefined();
    expect(notices.current?.outputLines).toBe(2000);
  });

  it("tells how an operation ended without counting it as an error", () => {
    notices.inform("Worktree repaired", "Git knows linked is at E:/w/linked again.");

    expect(notices.current).toMatchObject({ severity: "info", title: "Worktree repaired" });
    expect(notices.errorCount).toBe(0);
    notices.dismiss();
    expect(notices.current).toBeUndefined();
  });

  it("keeps a result behind every error and warning", async () => {
    await warn(goneWorktree);
    notices.inform("Worktree repaired", "done");
    errors.report(refusal("first"), "Could not merge");

    expect(notices.all.map((notice) => notice.severity)).toEqual(["error", "warning", "info"]);
    notices.dismissAll();
    expect(notices.all.map((notice) => notice.severity)).toEqual(["warning"]);
  });
});
