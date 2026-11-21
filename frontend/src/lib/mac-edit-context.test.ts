import { expect, it } from "vitest";
import { editRoles } from "./mac-edit-context";

it("keeps the native edit roles in text fields", () => {
  for (const surface of [null, "files", "solver"] as const) {
    expect(editRoles(surface, "field", false)).toEqual({ nativeSelectAll: true, nativeUndo: true });
  }
});

it("gives Files its selection and discard and the Solver its focused history", () => {
  expect(editRoles("files", "other", false)).toEqual({ nativeSelectAll: false, nativeUndo: false });
  expect(editRoles("solver", "editor", false)).toEqual({ nativeSelectAll: true, nativeUndo: false });
  expect(editRoles(null, "other", false)).toEqual({ nativeSelectAll: false, nativeUndo: true });
});

it("never dispatches an undo into a covered panel", () => {
  expect(editRoles("files", "other", true).nativeUndo).toBe(true);
  expect(editRoles("files", "output", false).nativeUndo).toBe(true);
  expect(editRoles("solver", "editor", true).nativeUndo).toBe(true);
});
