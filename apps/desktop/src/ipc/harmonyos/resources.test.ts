import { describe, expect, it } from "vitest";
import { isLocalNavigation, LOCAL_ENTRY, resolveLocalAsset } from "../../../../../platform/harmonyos/entry/src/main/ets/host/LocalResources";

describe("packaged HarmonyOS origin", () => {
  it("serves the entry and Vite chunks with module-compatible MIME types", () => {
    expect(resolveLocalAsset(LOCAL_ENTRY, "GET")).toEqual({ path: "web/harmonyos-app.html", mime: "text/html" });
    expect(resolveLocalAsset("https://hatoba.invalid/assets/chunk-123.js", "GET")?.mime).toBe("text/javascript");
    expect(resolveLocalAsset("https://hatoba.invalid/assets/style-123.css", "GET")?.mime).toBe("text/css");
  });
  it("denies external URLs, origin lookalikes, traversal, encoding and non-GET access", () => {
    for (const path of ["https://example.com/assets/app.js", "https://hatoba.invalid.evil/assets/app.js", "https://hatoba.invalid@evil/assets/app.js", "https://hatoba.invalid/assets/../secret.json", "https://hatoba.invalid/assets/%2e%2e/secret.json", "https://hatoba.invalid/assets/a%2fsecret.js", "https://hatoba.invalid/assets/a\\b.js", "https://hatoba.invalid/assets//a.js", "https://hatoba.invalid/secrets.json", "https://hatoba.invalid/assets/a.js?redirect=evil", "file:///etc/passwd"]) {
      expect(resolveLocalAsset(path, "GET")).toBeUndefined();
    }
    expect(resolveLocalAsset(LOCAL_ENTRY, "POST")).toBeUndefined();
  });
  it("permits only the packaged entry document and its fragments as navigation", () => {
    expect(isLocalNavigation(LOCAL_ENTRY)).toBe(true);
    expect(isLocalNavigation(`${LOCAL_ENTRY}#terminal`)).toBe(true);
    expect(isLocalNavigation(`${LOCAL_ENTRY}/evil`)).toBe(false);
    expect(isLocalNavigation("https://hatoba.invalid/assets/untrusted.html")).toBe(false);
    expect(isLocalNavigation("https://example.com")).toBe(false);
  });
});
