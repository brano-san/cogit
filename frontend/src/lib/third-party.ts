/** The frontend half of Help ▸ About ▸ Third-party licences: which npm packages the Vite
    build actually bundled. Run by the build (vite.config.ts), never in the window. */

export const THIRD_PARTY_FILE = "third-party-licences.txt";

export interface BundledPackage {
  name: string;
  version: string;
  license: string;
  repository: string | null;
}

const MARKER = "/node_modules/";

/** The package directory a bundled module lives in, or null for Cogit's own code. */
export function packageRoot(moduleId: string): string | null {
  const id = moduleId.replace(/^\0/, "").split("?")[0]!.replace(/\\/g, "/");
  const at = id.lastIndexOf(MARKER);
  if (at < 0) return null;
  const rest = id.slice(at + MARKER.length).split("/");
  const depth = rest[0]!.startsWith("@") ? 2 : 1;
  if (rest.length <= depth) return null;
  return id.slice(0, at + MARKER.length) + rest.slice(0, depth).join("/");
}

/** `[module id, rendered length]` pairs: a module tree-shaken to nothing ships nothing. */
export function bundledRoots(modules: Iterable<[string, number]>): string[] {
  const roots = new Set<string>();
  for (const [id, length] of modules) {
    const root = length > 0 ? packageRoot(id) : null;
    if (root) roots.add(root);
  }
  return [...roots];
}

function licenseOf(manifest: Record<string, unknown>): string {
  const { license, licenses } = manifest;
  if (typeof license === "string") return license;
  if (license && typeof license === "object" && "type" in license) return String(license.type);
  if (Array.isArray(licenses)) {
    const types = licenses.map((entry) => (entry as { type?: unknown }).type).filter(Boolean);
    if (types.length > 0) return types.join(" OR ");
  }
  return "unknown";
}

function repositoryOf(manifest: Record<string, unknown>): string | null {
  const { repository } = manifest;
  const raw =
    typeof repository === "string"
      ? repository
      : repository && typeof repository === "object" && "url" in repository
        ? String(repository.url)
        : null;
  if (!raw) return null;
  const url = raw.replace(/^git\+/, "").replace(/\.git$/, "").replace(/^github:/, "");
  return /^[\w.-]+\/[\w.-]+$/.test(url) ? `https://github.com/${url}` : url;
}

export function describePackage(manifest: Record<string, unknown>): BundledPackage | null {
  if (typeof manifest.name !== "string" || manifest.name === "") return null;
  return {
    name: manifest.name,
    version: typeof manifest.version === "string" ? manifest.version : "?",
    license: licenseOf(manifest),
    repository: repositoryOf(manifest),
  };
}

export function renderPackages(packages: readonly BundledPackage[]): string {
  const sorted = [...packages].sort((a, b) => a.name.localeCompare(b.name, "en"));
  const lines = sorted.map(
    (entry) =>
      `${entry.name} ${entry.version} — ${entry.license}` +
      (entry.repository ? ` — ${entry.repository}` : ""),
  );
  return [`Frontend packages (${sorted.length})`, "", ...lines, ""].join("\n");
}
