#!/usr/bin/env python3
"""Restart only the installed P0 app and verify its real Web -> Rust -> Web probe.

The evidence contains build metadata and generated-data checks, never device IDs or full logs.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
bundle = json.loads((ROOT / "platform/harmonyos/AppScope/app.json5").read_text())["app"]["bundleName"]
ability = json.loads((ROOT / "platform/harmonyos/entry/src/main/module.json5").read_text())["module"]["mainElement"]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target", required=True, help="Connected hdc target")
parser.add_argument("--hdc", default=str(Path(os.environ.get("HATOBA_DEVECO_HOME", "/Applications/DevEco-Studio.app/Contents")) / "sdk/default/openharmony/toolchains/hdc"))
parser.add_argument("--device-label", default="Unspecified development device")
parser.add_argument("--system-label", default="Unspecified HarmonyOS version")
parser.add_argument("--output", type=Path)
args = parser.parse_args()
hdc = [args.hdc, "-t", args.target]


def command(*parts):
    result = subprocess.run(hdc + list(parts), text=True, capture_output=True, timeout=15, check=True)
    return result.stdout


if "successfully" not in command("shell", "aa", "force-stop", bundle):
    raise RuntimeError("Could not stop the P0 app")
if "start ability successfully" not in command("shell", "aa", "start", "-a", ability, "-b", bundle):
    raise RuntimeError("Could not start the P0 app")
report = None
rendered = False
deadline = time.monotonic() + 12
while time.monotonic() < deadline:
    pid = command("shell", "pidof", bundle).strip()
    if pid.isdigit():
        log = command("shell", "hilog", "-x", "-P", pid, "-T", "OH-Hatoba")
        for line in log.splitlines():
            marker = "P0 native probe: "
            if marker in line:
                report = json.loads(line.split(marker, 1)[1])
            if "P0 Web received and rendered native probe" in line:
                rendered = True
        if report is not None and rendered:
            break
    time.sleep(0.25)
if report is None or not rendered:
    raise RuntimeError("Did not observe both the native report and Web rendering confirmation; device validation failed")
baseline = json.loads((ROOT / "platform/harmonyos/upstream.json").read_text())
assert report["host_protocol"] == baseline["hostProtocol"], "Host protocol mismatch"
assert report["upstream_commit"] == baseline["commit"], "Installed app is not the recorded upstream baseline"
assert report["upstream_version"] == baseline["version"], "Installed version mismatch"
assert (report["target_os"], report["target_env"], report["architecture"]) == ("linux", "ohos", "aarch64"), "This is not the OHOS ARM64 native host"
expected_checks = {"random_aead_and_tamper_rejection", "sqlite_schema", "ssh_ed25519_roundtrip", "tokio_runtime", "ai_provider_url_validation"}
assert {check["name"] for check in report["checks"]} == expected_checks, "Probe checks changed; review the validation script"
assert all(check["passed"] is True for check in report["checks"]), "Native dependency probe failed"
# Rebuild a narrow output schema rather than persisting raw application logs.
evidence = {
    "verifiedAt": datetime.now(timezone.utc).isoformat(),
    "device": args.device_label,
    "system": args.system_label,
    "webReceivedAndRendered": rendered,
    "native": {key: report[key] for key in ["host_protocol", "upstream_version", "upstream_commit", "architecture", "target_os", "target_env"]},
    "checks": [{"name": check["name"], "passed": check["passed"]} for check in report["checks"]],
    "scope": "P0 generated-data dependency probe; does not validate SSH networking, TLS, IME or lifecycle recovery",
}
encoded = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
if args.output:
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(encoded)
print(encoded, end="")
