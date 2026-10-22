import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const root = fileURLToPath(new URL("../../", import.meta.url));
const tree = execFileSync("cargo", ["tree", "--locked", "--offline", "-p", "hatoba-ohos", "--target", "aarch64-unknown-linux-ohos", "--edges", "normal", "--prefix", "none", "--format", "{p} features=[{f}]"], { cwd: root, encoding: "utf8" });
const forbidden = new Set(["tauri", "tauri-runtime", "tauri-runtime-wry", "wry", "tao", "gtk", "gdk", "zbus", "keyring", "hatoba-local", "hatoba-cli"]);
const found = [...new Set(tree.split("\n").map((line) => line.split(" ")[0]).filter((name) => forbidden.has(name)))];
if (found.length) throw new Error(`Desktop dependencies entered the OHOS host: ${found.join(", ")}`);
if (tree.split("\n").some((line) => line.startsWith("hatoba-core ") && /features=\[[^\]]*\btest-util\b/.test(line))) throw new Error("Production native host must not enable hatoba-core/test-util");
const module = JSON.parse(fs.readFileSync(path.join(root, "platform/harmonyos/entry/src/main/module.json5"), "utf8")).module;
if (JSON.stringify([...module.deviceTypes].sort()) !== JSON.stringify(["2in1", "tablet"])) throw new Error("Keep both 2in1 and tablet device types");
const profile = JSON.parse(fs.readFileSync(path.join(root, "platform/harmonyos/build-profile.json5"), "utf8"));
if (profile.app.signingConfigs?.length || profile.app.products.some((product) => product.signingConfig)) throw new Error("Move signing credentials out of the tracked profile: node scripts/harmonyos/local-signing.mjs capture");
for (const product of profile.app.products) {
  if (product.compatibleSdkVersion !== "6.0.0(20)" || product.compileSdkVersion !== "26.0.0" || product.targetSdkVersion !== "26.0.0") throw new Error("SDK baseline drift: keep min API20, compile/target API26");
}
console.log("HarmonyOS boundaries checked: no desktop runtime dependencies; 2in1 + tablet");
