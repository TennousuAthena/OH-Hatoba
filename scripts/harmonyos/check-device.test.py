"""Replay generated hdc logs to check freshness and ring-buffer rollover without a device."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DeviceEvidenceTests(unittest.TestCase):
    def run_case(self, mode, passes):
        baseline = json.loads((ROOT / "platform/harmonyos/upstream.json").read_text())
        prior = json.loads((ROOT / "platform/harmonyos/docs/evidence/2026-11-13-tablet-upstream-085-network.json").read_text())
        report = {**prior["native"], "upstream_version": baseline["version"],
                  "upstream_commit": baseline["commit"], "checks": prior["checks"]}
        early = "\n".join(["P0 native probe: " + json.dumps(report),
                            "P0 Web received and rendered native probe",
                            "Shared business smoke passed and rendered"])
        late = "\n".join(["Native network probe: " + json.dumps({"checks": prior["networkChecks"]}),
                           "Web SSH transport passed", "Web Telnet transport passed",
                           "Web flow-control budget recovered", "Web AI assistant lifecycle passed",
                           "Network probe passed and rendered"])
        with tempfile.TemporaryDirectory(prefix="hatoba-validator-") as directory:
            fixture = Path(directory) / "hdc"
            state = Path(directory) / "state.json"
            fixture.write_text("""#!/usr/bin/env python3
import json,sys
from pathlib import Path
state=Path(%r); mode=%r; early=%r; late=%r
a=sys.argv
if 'force-stop' in a: print('successfully')
elif 'start' in a:
 state.write_text(json.dumps({'run':a[a.index('ohhatoba.validationRun')+1],'reads':0}))
 print('start ability successfully')
elif 'pidof' in a:
 s=json.loads(state.read_text())
 print('456' if mode=='restart' and s['reads'] else '123')
elif 'hilog' in a:
 s=json.loads(state.read_text()); s['reads']+=1; state.write_text(json.dumps(s))
 marker='Validation run started: '+s['run']
 if mode=='stale': print('Validation run started: '+'0'*32+'\\n'+early+'\\n'+late+'\\n'+marker+'\\nDiagnostic failed')
 elif mode in ('rolled','restart'):
  print(marker+'\\n'+early if s['reads']==1 else late)
 else: print(marker+'\\n'+early+'\\n'+late+('\\nDiagnostic failed' if mode=='error' else ''))
""" % (str(state), mode, early, late))
            fixture.chmod(0o700)
            result = subprocess.run(["python3", str(ROOT / "scripts/harmonyos/check-device.py"),
                                     "--hdc", str(fixture), "--target", "generated-fixture", "--network"],
                                    capture_output=True, text=True, timeout=5, cwd=ROOT)
            self.assertEqual(result.returncode == 0, passes, mode)

    def test_current_success(self):
        self.run_case("success", True)

    def test_current_error_overrides_success(self):
        self.run_case("error", False)

    def test_stale_success_cannot_mask_current_failure(self):
        self.run_case("stale", False)

    def test_ring_buffer_can_evict_observed_start_marker(self):
        self.run_case("rolled", True)

    def test_process_restart_cannot_combine_two_runs(self):
        self.run_case("restart", False)


if __name__ == "__main__":
    unittest.main()
