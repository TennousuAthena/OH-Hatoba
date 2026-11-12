import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const buildScript = fileURLToPath(new URL("./build-hap.sh", import.meta.url));
const outputs = "platform/harmonyos/entry/build/default/outputs/default";
const unsignedName = "entry-default-unsigned.hap";
const signedName = "entry-default-signed.hap";

// Every external build tool is a recorder. No repository signing configuration, SDK or HAP
// is read: the real shell script runs only against a disposable directory and fake tools.
const recorder = `
import fs from "node:fs";
import path from "node:path";
const [name, ...args] = process.argv.slice(2);
const root = process.env.MOCK_HATOBA_ROOT;
const output = path.join(root, ${JSON.stringify(outputs)});
const unsigned = path.join(output, ${JSON.stringify(unsignedName)});
const signed = path.join(output, ${JSON.stringify(signedName)});
const call = {
  name, args,
  mode: process.env.HATOBA_BUILD_MODE,
  unsigned: process.env.HATOBA_UNSIGNED,
  nodeHome: process.env.NODE_HOME,
  pathHead: process.env.PATH.split(path.delimiter)[0],
};
if (name === "hvigor") {
  call.staleUnsigned = fs.existsSync(unsigned);
  call.staleSigned = fs.existsSync(signed);
  if (process.env.MOCK_HATOBA_EMIT_UNSIGNED !== "0") fs.writeFileSync(unsigned, "fresh unsigned");
  const configured = fs.existsSync(path.join(root, "platform/harmonyos/build-profile.local.json5"));
  const emitSigned = process.env.MOCK_HATOBA_EMIT_SIGNED;
  if (emitSigned === "1" || (emitSigned !== "0" && configured && call.unsigned !== "1")) {
    fs.writeFileSync(signed, "fresh signed");
  }
}
fs.appendFileSync(path.join(root, "calls.jsonl"), JSON.stringify(call) + "\\n");
if (name === "python3") {
  const requested = args.slice(1).find(arg => !arg.startsWith("--"));
  const artifact = requested ? path.resolve(requested) : unsigned;
  if (!fs.existsSync(artifact) || fs.readFileSync(artifact, "utf8").startsWith("stale")) {
    process.stderr.write("Missing fresh test artifact\\n");
    process.exitCode = 1;
  }
}
`;

function shellQuote(value) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function fixture(t, { localSigning = true, emitSigned, emitUnsigned = true } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hatoba-build-hap-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const bin = path.join(root, "fake-bin");
  const deveco = path.join(root, "fake-deveco");
  const hostNodeHome = path.join(root, "fake-host-node");
  const bundledNodeHome = path.join(deveco, "tools/node");
  const script = path.join(root, "scripts/harmonyos/build-hap.sh");
  const runner = path.join(root, "record-tool.mjs");
  const output = path.join(root, outputs);
  fs.mkdirSync(path.dirname(script), { recursive: true });
  fs.mkdirSync(output, { recursive: true });
  fs.copyFileSync(buildScript, script);
  fs.writeFileSync(runner, recorder);
  if (localSigning) {
    fs.writeFileSync(path.join(root, "platform/harmonyos/build-profile.local.json5"), '{"fixture":true}');
  }
  // A prior signed/debug build must never satisfy the new build's artifact checks.
  for (const name of [unsignedName, signedName]) fs.writeFileSync(path.join(output, name), "stale");
  function tool(relative, name) {
    const file = path.join(root, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, `#!/bin/sh\nexec ${shellQuote(process.execPath)} ${shellQuote(runner)} ${shellQuote(name)} "$@"\n`, { mode: 0o755 });
  }
  for (const name of ["node", "pnpm", "python3"]) tool(`fake-bin/${name}`, name);
  tool("scripts/harmonyos/build-native.sh", "native");
  tool("fake-deveco/tools/ohpm/bin/ohpm", "ohpm");
  tool("fake-deveco/tools/hvigor/bin/hvigorw", "hvigor");
  fs.mkdirSync(path.join(bundledNodeHome, "bin"), { recursive: true });
  fs.writeFileSync(path.join(bundledNodeHome, "bin/node"), `#!/bin/sh\nexec ${shellQuote(process.execPath)} "$@"\n`, { mode: 0o755 });
  return {
    output,
    run(args = []) {
      const result = spawnSync("/bin/sh", [script, ...args], {
        cwd: root,
        env: {
          ...process.env,
          PATH: `${bin}:/usr/bin:/bin`,
          NODE_HOME: hostNodeHome,
          HATOBA_DEVECO_HOME: deveco,
          DEVECO_SDK_HOME: path.join(deveco, "sdk"),
          HATOBA_OHOS_NDK: path.join(deveco, "sdk/default/openharmony/native"),
          MOCK_HATOBA_ROOT: root,
          MOCK_HATOBA_EMIT_SIGNED: emitSigned === undefined ? "auto" : String(Number(emitSigned)),
          MOCK_HATOBA_EMIT_UNSIGNED: String(Number(emitUnsigned)),
        },
        encoding: "utf8",
        timeout: 15_000,
      });
      assert.ifError(result.error);
      const calls = fs.readFileSync(path.join(root, "calls.jsonl"), "utf8").trim().split("\n").map(JSON.parse);
      return { ...result, calls, hostNodeHome, bundledNodeHome, bin };
    },
  };
}

function named(result, name) {
  return result.calls.filter(call => call.name === name);
}

function assertFreshBuild(result, mode, unsigned) {
  const [hvigor] = named(result, "hvigor");
  assert.equal(hvigor.staleUnsigned, false);
  assert.equal(hvigor.staleSigned, false);
  assert.ok(hvigor.args.includes(`buildMode=${mode}`));
  assert.equal(hvigor.mode, mode);
  assert.equal(hvigor.unsigned, unsigned);
  assert.equal(named(result, "node").length, 2);
  assert.equal(named(result, "pnpm").length, 2);
  for (const call of result.calls) {
    const sdkTool = call.name === "ohpm" || call.name === "hvigor";
    assert.equal(call.nodeHome, sdkTool ? result.bundledNodeHome : result.hostNodeHome, call.name);
    assert.equal(call.pathHead, sdkTool ? path.join(result.bundledNodeHome, "bin") : result.bin, call.name);
  }
}

test("default debug build uses local signing and checks both fresh artifacts", t => {
  const result = fixture(t).run();
  assert.equal(result.status, 0, result.stderr);
  assertFreshBuild(result, "debug", "0");
  assert.deepEqual(named(result, "native")[0].args, ["--locked"]);
  const checks = named(result, "python3");
  assert.equal(checks.length, 2);
  assert.ok(checks[0].args[1].endsWith(unsignedName));
  assert.ok(checks[0].args.includes("--require-unsigned"));
  assert.ok(checks[1].args[1].endsWith(signedName));
  assert.ok(checks.every(call => !call.args.includes("--require-release")));
});

test("release unsigned overrides local signing and forwards release mode to each tool", t => {
  const f = fixture(t);
  const result = f.run(["--release", "--unsigned", "--", "--stacktrace"]);
  assert.equal(result.status, 0, result.stderr);
  assertFreshBuild(result, "release", "1");
  const [native] = named(result, "native");
  assert.deepEqual(native.args, ["--locked", "--release"]);
  assert.equal(native.mode, "release");
  assert.equal(native.unsigned, "1");
  const [hvigor] = named(result, "hvigor");
  assert.equal(hvigor.args.at(-1), "--stacktrace");
  assert.ok(!hvigor.args.includes("--release") && !hvigor.args.includes("--unsigned"));
  const checks = named(result, "python3");
  assert.equal(checks.length, 1);
  assert.ok(checks[0].args[1].endsWith(unsignedName));
  assert.ok(checks[0].args.includes("--require-release"));
  assert.ok(checks[0].args.includes("--require-unsigned"));
  assert.equal(fs.existsSync(path.join(f.output, signedName)), false);
});

test("release with local signing verifies release metadata for both artifacts", t => {
  const result = fixture(t).run(["--release"]);
  assert.equal(result.status, 0, result.stderr);
  assertFreshBuild(result, "release", "0");
  const checks = named(result, "python3");
  assert.equal(checks.length, 2);
  assert.ok(checks.every(call => call.args.includes("--require-release")));
});

test("default build without local signing requires only the unsigned artifact", t => {
  const result = fixture(t, { localSigning: false }).run();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(named(result, "python3").length, 1);
  assertFreshBuild(result, "debug", "0");
});

test("a stale signed HAP cannot hide a signing step that produced no output", t => {
  const result = fixture(t, { emitSigned: false }).run();
  assert.notEqual(result.status, 0);
  assertFreshBuild(result, "debug", "0");
  assert.equal(named(result, "python3").length, 2);
  assert.match(result.stderr, /Missing fresh test artifact/);
});

test("an unsigned build rejects an unexpectedly generated signed HAP", t => {
  const result = fixture(t, { emitSigned: true }).run(["--release", "--unsigned"]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unsigned build unexpectedly produced a signed HAP/);
  assert.equal(named(result, "python3").length, 1);
});

test("a stale unsigned HAP cannot hide a successful tool that produced no HAP", t => {
  const result = fixture(t, { emitUnsigned: false }).run(["--release", "--unsigned"]);
  assert.notEqual(result.status, 0);
  assertFreshBuild(result, "release", "1");
  assert.match(result.stderr, /Missing fresh test artifact/);
});

test("unsigned Hvigor evaluation clears cached signing links without touching local material", () => {
  const requireDesktop = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
  const ts = requireDesktop("typescript");
  const source = fs.readFileSync(new URL("../../platform/harmonyos/hvigorfile.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, esModuleInterop: true },
  });
  const callbacks = [];
  const profile = {
    app: {
      signingConfigs: [{ name: "fixture-only" }],
      products: [
        { name: "default", signingConfig: "fixture-only", compatibleSdkVersion: "6.0.0(20)" },
        { name: "other", signingConfig: "another-cached-link" },
      ],
    },
  };
  let writes = 0;
  const context = {
    getBuildProfileOpt: () => profile,
    setBuildProfileOpt: updated => { assert.equal(updated, profile); writes += 1; },
  };
  function noFileAccess() {
    assert.fail("Unsigned signing evaluation must not inspect local files");
  }
  const root = {
    afterNodeEvaluate: callback => callbacks.push(callback),
    getContext: id => { assert.equal(id, "fixture-app-plugin"); return context; },
    getNodePath: noFileAccess,
  };
  const modules = {
    "@ohos/hvigor": { hvigor: { getRootNode: () => root } },
    "@ohos/hvigor-ohos-plugin": { appTasks: {}, OhosPluginId: { OHOS_APP_PLUGIN: "fixture-app-plugin" } },
    "node:fs": { existsSync: noFileAccess, readFileSync: noFileAccess, writeFileSync: noFileAccess },
    "node:path": path,
  };
  vm.runInNewContext(outputText, {
    exports: {},
    require: name => { assert.ok(Object.hasOwn(modules, name), `Unexpected import: ${name}`); return modules[name]; },
    process: { env: { HATOBA_UNSIGNED: "1" } },
    console: { log() {} },
  });
  assert.equal(callbacks.length, 2);
  // The first callback synchronizes branding; this test exercises only the signing callback.
  callbacks[1]();
  assert.equal(writes, 1);
  assert.equal(Object.hasOwn(profile.app, "signingConfigs"), false);
  assert.ok(profile.app.products.every(product => !Object.hasOwn(product, "signingConfig")));
  assert.equal(profile.app.products[0].compatibleSdkVersion, "6.0.0(20)");
});
