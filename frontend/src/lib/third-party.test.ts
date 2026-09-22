import { describe, expect, it } from "vitest";
import { bundledRoots, describePackage, packageRoot, renderPackages } from "./third-party";

const NM = "C:/work/cogit/node_modules";

describe("packageRoot", () => {
  it("finds the package a bundled file belongs to", () => {
    expect(packageRoot(`${NM}/svelte/src/internal/client/index.js`)).toBe(`${NM}/svelte`);
  });

  it("keeps the scope of a scoped package", () => {
    expect(packageRoot(`${NM}/@codemirror/view/dist/index.js`)).toBe(`${NM}/@codemirror/view`);
  });

  it("takes the innermost of nested packages", () => {
    expect(packageRoot(`${NM}/a/node_modules/b/index.js`)).toBe(`${NM}/a/node_modules/b`);
  });

  it("reads Windows separators, virtual prefixes and queries", () => {
    expect(packageRoot("\0C:\\work\\node_modules\\esm-env\\index.js?commonjs")).toBe(
      "C:/work/node_modules/esm-env",
    );
  });

  it("ignores Cogit's own sources and virtual modules", () => {
    expect(packageRoot("C:/work/cogit/frontend/src/App.svelte")).toBeNull();
    expect(packageRoot("\0vite/preload-helper.js")).toBeNull();
  });
});

describe("bundledRoots", () => {
  it("lists each package once, and only for code that survived tree-shaking", () => {
    const roots = bundledRoots([
      [`${NM}/svelte/src/a.js`, 120],
      [`${NM}/svelte/src/b.js`, 80],
      [`${NM}/unused/index.js`, 0],
      ["C:/work/cogit/frontend/src/main.ts", 50],
    ]);
    expect(roots).toEqual([`${NM}/svelte`]);
  });
});

describe("describePackage", () => {
  it("reads a plain licence and a repository object", () => {
    const found = describePackage({
      name: "@codemirror/view",
      version: "6.43.12",
      license: "MIT",
      repository: { type: "git", url: "git+https://github.com/codemirror/view.git" },
    });
    expect(found).toEqual({
      name: "@codemirror/view",
      version: "6.43.12",
      license: "MIT",
      repository: "https://github.com/codemirror/view",
    });
  });

  const licence = (manifest: Record<string, unknown>) =>
    describePackage({ name: "a", version: "1", ...manifest })?.license;
  const source = (repository: unknown) =>
    describePackage({ name: "a", version: "1", repository })?.repository;

  it("reads the old object and list forms of the licence", () => {
    expect(licence({ license: { type: "ISC" } })).toBe("ISC");
    expect(licence({ licenses: [{ type: "MIT" }, { type: "Apache-2.0" }] })).toBe(
      "MIT OR Apache-2.0",
    );
    expect(licence({})).toBe("unknown");
  });

  it("expands the GitHub shorthand of a repository", () => {
    expect(source("sveltejs/svelte")).toBe("https://github.com/sveltejs/svelte");
    expect(source("github:lezer-parser/lr")).toBe("https://github.com/lezer-parser/lr");
  });

  it("rejects a manifest with no name", () => {
    expect(describePackage({ version: "1" })).toBeNull();
  });
});

describe("renderPackages", () => {
  it("is one line per package under a count, sorted by name", () => {
    const lr = "https://github.com/lezer-parser/lr";
    const text = renderPackages([
      { name: "svelte", version: "5.57.1", license: "MIT", repository: null },
      { name: "@lezer/lr", version: "1.4.2", license: "MIT", repository: lr },
    ]);
    expect(text.split("\n")).toEqual([
      "Frontend packages (2)",
      "",
      "@lezer/lr 1.4.2 — MIT — https://github.com/lezer-parser/lr",
      "svelte 5.57.1 — MIT",
      "",
    ]);
  });
});
