import type { Environment } from "vitest/runtime";

/** Node, but transformed for the client: Svelte compiles `$effect` to a no-op for the
    server, and store tests that need effects run (`*.svelte.test.ts`) must not get that. */
export default <Environment>{
  name: "node-runes",
  viteEnvironment: "client",
  setup() {
    return { teardown() {} };
  },
};
