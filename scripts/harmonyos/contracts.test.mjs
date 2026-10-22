import fs from "node:fs";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import test from "node:test";
import { readContract } from "./contracts.mjs";
import { compareContracts } from "./upstream.mjs";

const root = new URL("../../", import.meta.url);
const read = (relative) => fs.readFileSync(fileURLToPath(new URL(relative, root)), "utf8");

test("contract reader observes real parameter renames without guessing casing", () => {
  const before = readContract(read);
  const after = readContract((file) => {
    const text = read(file);
    return file.endsWith("bindings.ts")
      ? text.replace('hostGet: (id: string) => __TAURI_INVOKE<HostView>("host_get", { id })', 'hostGet: (hostId: string) => __TAURI_INVOKE<HostView>("host_get", { hostId })')
      : text;
  });
  assert.deepEqual(compareContracts(before, after).commands.changed, ["hostGet"]);
  assert.deepEqual(after.commands.hostGet.parameters, ["hostId"]);
});

test("field changes behind an unchanged named DTO require review", () => {
  const before = readContract(read);
  const after = readContract((file) => file.endsWith("bindings.ts") ? read(file).replace("export type AppInfo = {", "export type AppInfo = { hostProtocol: number;") : read(file));
  assert.ok(compareContracts(before, after).dtos.changed.includes("AppInfo"));
});

test("a changed bindings generator is rejected instead of silently misrouting calls", () => {
  assert.throws(() => readContract((file) => file.endsWith("bindings.ts") ? read(file).replaceAll("__TAURI_INVOKE<", "otherInvoke<") : read(file)), /unsupported invoke expression/);
});

test("added and removed commands/events are visible to upgrade review", () => {
  const result = compareContracts({ commands: { old: {} }, events: { removed: "A" }, dtos: {} }, { commands: { next: {} }, events: { added: "B" }, dtos: {} });
  assert.deepEqual(result.commands, { added: ["next"], removed: ["old"], changed: [] });
  assert.deepEqual(result.events, { added: ["added"], removed: ["removed"], changed: [] });
});
