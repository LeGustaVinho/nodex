// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

/**
 * Every diagram the Rust tests generate (`crates/nodex-uml/tests/mmd`) must parse with the
 * Mermaid version the app ships.
 */
const files = import.meta.glob<string>("../../../crates/nodex-uml/tests/mmd/*.mmd", { query: "?raw", import: "default", eager: true });

describe("generated UML diagrams", () => {
  it("exist", () => expect(Object.keys(files).length).toBeGreaterThan(5));
  for (const [path, text] of Object.entries(files)) {
    it(`${path.split("/").pop()} parses`, async () => {
      const { default: mermaid } = await import("mermaid");
      mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
      let error = "";
      try {
        await mermaid.parse(text);
      } catch (e) {
        error = String(e);
      }
      expect(error).toBe("");
    });
  }
});
