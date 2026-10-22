import { fileURLToPath, URL } from "node:url";
import { mergeConfig } from "vite";
import upstream from "./vite.config.ts";

export default mergeConfig(upstream, {
  base: "./",
  build: {
    outDir: "../../platform/harmonyos/entry/src/main/resources/rawfile/web",
    emptyOutDir: true,
    rollupOptions: { input: fileURLToPath(new URL("./harmonyos-probe.html", import.meta.url)) },
  },
});
