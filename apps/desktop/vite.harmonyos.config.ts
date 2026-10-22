import { fileURLToPath, URL } from "node:url";
import { mergeConfig } from "vite";
import upstream from "./vite.config.ts";

export default mergeConfig(upstream, {
  base: "./",
  build: {
    outDir: "../../platform/harmonyos/entry/src/main/resources/rawfile/web",
    emptyOutDir: true,
    // Keep entry side effects out of shared chunks (including Rolldown's runtime helpers).
    // Otherwise loading the diagnostics bridge can also execute the production app's boot.
    rollupOptions: { preserveEntrySignatures: "strict", input: {
      app: fileURLToPath(new URL("./harmonyos-app.html", import.meta.url)),
      diagnostics: fileURLToPath(new URL("./harmonyos-probe.html", import.meta.url)),
    } },
  },
});
