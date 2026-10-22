import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { readContract } from "./contracts.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const baseline = JSON.parse(fs.readFileSync(path.join(root, "platform/harmonyos/upstream.json"), "utf8"));

function git(...args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024, stdio: ["ignore", "pipe", "pipe"] }).trimEnd();
}

export function compareContracts(before, after) {
  const result = {};
  for (const group of ["commands", "events", "dtos"]) {
    const old = before[group] ?? {};
    const next = after[group] ?? {};
    result[group] = {
      added: Object.keys(next).filter((key) => !(key in old)),
      removed: Object.keys(old).filter((key) => !(key in next)),
      changed: Object.keys(next).filter((key) => key in old && JSON.stringify(old[key]) !== JSON.stringify(next[key])),
    };
  }
  return result;
}

export function audit(ref = "upstream/main") {
  if (!ref || ref.startsWith("-")) throw new Error("Expected a Git commit, tag or branch");
  if (!/^[a-f0-9]{40}$/.test(baseline.commit)) throw new Error("Invalid recorded upstream baseline");
  const candidate = git("rev-parse", "--verify", `${ref}^{commit}`);
  const load = (commit) => readContract((relative) => git("show", `${commit}:${relative}`));
  const changes = compareContracts(load(baseline.commit), load(candidate));
  const files = git("diff", "--name-only", baseline.commit, candidate, "--", "Cargo.toml", "Cargo.lock", "package.json", "pnpm-workspace.yaml", "pnpm-lock.yaml", "crates/", "apps/desktop/", "workers/sync/", "scripts/worker/").split("\n").filter(Boolean);
  return {
    baseline: baseline.commit,
    candidate,
    changes,
    reviewFiles: files,
    requiresReview: files.length > 0 || Object.values(changes).some((group) => Object.values(group).some((items) => items.length)),
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const ref = process.argv.slice(2).find((arg) => !arg.startsWith("--"));
  const result = audit(ref);
  console.log(JSON.stringify(result, null, 2));
  if (process.argv.includes("--check") && result.requiresReview) process.exitCode = 1;
}
