import assert from "node:assert/strict";
import test from "node:test";
import { separateSigning } from "./local-signing.mjs";

test("separates credentials without replacing shared SDK policy", () => {
  const input = { app: { signingConfigs: [{ name: "debug", material: { storePassword: "fixture-only" } }], products: [{ name: "default", signingConfig: "debug", compatibleSdkVersion: "6.0.0(20)" }] }, modules: [{ name: "entry" }] };
  const { profile, local } = separateSigning(input);
  assert.equal(profile.app.signingConfigs, undefined);
  assert.equal(profile.app.products[0].signingConfig, undefined);
  assert.equal(profile.app.products[0].compatibleSdkVersion, "6.0.0(20)");
  assert.deepEqual(local.app.products, [{ name: "default", signingConfig: "debug" }]);
  assert.equal(local.app.signingConfigs[0].material.storePassword, "fixture-only");
  assert.equal(input.app.signingConfigs[0].material.storePassword, "fixture-only");
  assert.throws(() => separateSigning({ app: { products: [] } }), /No signing configuration/);
});
