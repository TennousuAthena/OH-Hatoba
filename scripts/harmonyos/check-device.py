#!/usr/bin/env python3
"""Restart only the installed app and verify its real Web -> Rust -> Web probe.

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
parser.add_argument("--window-surface", action="store_true", help="Require native window background synchronization in normal application mode")
parser.add_argument("--app", action="store_true", help="Verify normal application boot and React rendering without diagnostics")
parser.add_argument("--network", action="store_true", help="Require the disposable loopback SSH/SFTP and untrusted TLS fixtures through hdc rport")
args = parser.parse_args()
if args.window_surface and not args.app:
    parser.error("--window-surface requires --app")
if args.app and args.network:
    parser.error("--app and --network are separate checks")
hdc = [args.hdc, "-t", args.target]


def command(*parts):
    result = subprocess.run(hdc + list(parts), text=True, capture_output=True, timeout=15, check=True)
    return result.stdout


if "successfully" not in command("shell", "aa", "force-stop", bundle):
    raise RuntimeError("Could not stop the app")
launch = ["shell", "aa", "start", "-a", ability, "-b", bundle]
if not args.app:
    launch += ["--pb", "ohhatoba.diagnostics", "true"]
if args.network:
    launch += ["--pb", "ohhatoba.networkTests", "true"]
started = command(*launch)
if "start ability successfully" not in started:
    raise RuntimeError("Could not start the app: " + started.strip())
report = None
rendered = False
business = False
network = None
network_rendered = False
web_terminal = False
diagnostic_failed = False
app_boot = False
app_rendered = False
window_surface = False
console_error = False
deadline = time.monotonic() + (80 if args.network else 30)
while time.monotonic() < deadline:
    pid = command("shell", "pidof", bundle).strip()
    if pid.isdigit():
        log = command("shell", "hilog", "-x", "-P", pid, "-T", "OH-Hatoba")
        for line in log.splitlines():
            app_boot |= "Application boot completed" in line
            app_rendered |= "Application rendered" in line
            window_surface |= "Window surface synchronized" in line
            console_error |= "Web console error" in line or "Native host initialization failed" in line
            marker = "P0 native probe: "
            if marker in line:
                report = json.loads(line.split(marker, 1)[1])
            if "P0 Web received and rendered native probe" in line:
                rendered = True
            if "Shared business smoke passed and rendered" in line:
                business = True
            if "Native network probe: " in line:
                network = json.loads(line.split("Native network probe: ", 1)[1])
            if "Network probe passed and rendered" in line:
                network_rendered = True
            if "Web SSH transport passed" in line:
                web_terminal = True
            if "Diagnostic failed" in line:
                diagnostic_failed = True
        if args.app and app_boot and app_rendered and (not args.window_surface or window_surface):
            break
        if report is not None and rendered and business and (not args.network or (network is not None and (network_rendered or diagnostic_failed))):
            break
    time.sleep(0.25)
if args.app:
    assert app_boot and app_rendered and not console_error, "Normal application boot/render check failed"
    assert not args.window_surface or window_surface, "Native window background was not synchronized"
    evidence = {"verifiedAt": datetime.now(timezone.utc).isoformat(), "device": args.device_label,
                "system": args.system_label, "normalApplicationBoot": app_boot, "reactRendered": app_rendered,
                "scope": "Normal application entry loads its real native runtime, host capabilities, vault status and React application. No projection or UI interaction; does not prove layout, IME, touch or biometric usability."}
    if args.window_surface:
        evidence["nativeWindowSurfaceSynchronized"] = window_surface
    encoded = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
    print(encoded, end="")
    raise SystemExit(0)
if report is None or not rendered or not business:
    raise RuntimeError("Did not observe both the native report and Web rendering confirmation; device validation failed")
baseline = json.loads((ROOT / "platform/harmonyos/upstream.json").read_text())
assert report["host_protocol"] == baseline["hostProtocol"], "Host protocol mismatch"
assert report["upstream_commit"] == baseline["commit"], "Installed app is not the recorded upstream baseline"
assert report["upstream_version"] == baseline["version"], "Installed version mismatch"
assert (report["target_os"], report["target_env"], report["architecture"]) == ("linux", "ohos", "aarch64"), "This is not the OHOS ARM64 native host"
expected_checks = {"random_aead_and_tamper_rejection", "sqlite_schema", "ssh_ed25519_roundtrip", "tokio_runtime", "ai_provider_url_validation"}
assert {check["name"] for check in report["checks"]} == expected_checks, "Probe checks changed; review the validation script"
assert all(check["passed"] is True for check in report["checks"]), "Native dependency probe failed"
if args.network:
    expected_network = {"systemCaHttps", "untrustedCertificateRejected", "sshHostKeyRejection", "sshHandshakeAndAuthentication", "sshExec", "ptyBinaryUnicodeAndResize", "sftpUploadDownloadDocumentGrantsAndCancel", "localForward", "dynamicForward", "remoteForward", "aiHttpModels", "aiStreamingUnicodeAndCancellation", "mcpHttpInitializeListCallAndShutdown", "oauthPkceStateCallbackAndCancellation", "documentConfigSkillZipFolderAndGrantRevocation"}
    assert network, "Network report did not complete"
    assert set(network["checks"]) == expected_network, "Network checks changed; review the validator"
    assert all(value is True for value in network["checks"].values()), f"Native network checks failed: {network['checks']}"
    assert network_rendered, "Network report did not render"
    assert web_terminal, "Web SSH terminal transport did not pass"
# Rebuild a narrow output schema rather than persisting raw application logs.
evidence = {
    "verifiedAt": datetime.now(timezone.utc).isoformat(),
    "device": args.device_label,
    "system": args.system_label,
    "webReceivedAndRendered": rendered,
    "sharedBusinessSmoke": business,
    "native": {key: report[key] for key in ["host_protocol", "upstream_version", "upstream_commit", "architecture", "target_os", "target_env"]},
    "checks": [{"name": check["name"], "passed": check["passed"]} for check in report["checks"]],
    "scope": "Native dependencies plus real typed-facade vault create/reopen, host CRUD, agent configuration preservation, lock event, denied locked read and unlock. Does not validate SSH networking, TLS, IME or lifecycle recovery.",
}
if args.network:
    evidence["networkChecks"] = network["checks"]
    evidence["webSshTerminalTransport"] = web_terminal
    evidence["scope"] = "Real OHOS native TLS trust/rejection, SSH authentication/host-key rejection/exec/PTY binary Unicode/resize, SFTP upload/download through descriptor grants with byte comparison and cancellation, local/remote/dynamic forwards, AI streaming and cancellation, HTTP MCP, OAuth PKCE/state callback and cancellation, descriptor-based config/skill imports and grant revocation, plus shared business IPC smoke. Controlled generated-data fixtures; does not validate actual picker interaction, UI/IME, external account sign-in, or lifecycle recovery."
encoded = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
if args.output:
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(encoded)
print(encoded, end="")
