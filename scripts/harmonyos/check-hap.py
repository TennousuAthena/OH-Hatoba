#!/usr/bin/env python3
"""Check the actual packaged metadata and ELF, not just the source configuration."""
import json
from pathlib import Path
import struct
import sys
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[2]
hap = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "platform/harmonyos/entry/build/default/outputs/default/entry-default-unsigned.hap"
expected = json.loads((ROOT / "platform/harmonyos/AppScope/app.json5").read_text())["app"]
desktop = ROOT / "apps/desktop/src-tauri"
product_name = json.loads((desktop / "tauri.conf.json").read_text())["productName"]
strings = json.loads((ROOT / "platform/harmonyos/AppScope/resources/base/element/string.json").read_text())["string"]
assert next(item["value"] for item in strings if item["name"] == "app_name") == product_name, "Application name differs from upstream"
with ZipFile(hap) as archive:
    metadata = json.loads(archive.read("module.json"))
    app = metadata["app"]
    module = metadata["module"]
    # HarmonyOS encodes the declared product version ahead of the API in this field.
    assert app["minAPIVersion"] in (20, 60000020), "Packaged minimum API drifted"
    assert app["targetAPIVersion"] in (26, 260000026), "Packaged target API drifted"
    assert sorted(module["deviceTypes"]) == ["2in1", "tablet"], "Packaged device types drifted"
    for key in ("bundleName", "versionName", "versionCode"):
        assert app[key] == expected[key], f"Packaged {key} differs from source"
    library = archive.read("libs/arm64-v8a/libhatoba.so")
    assert library[:6] == b"\x7fELF\x02\x01", "Native host must be a 64-bit little-endian ELF"
    assert struct.unpack_from("<H", library, 18)[0] == 183, "Native host must be AArch64"
    for entry in ("harmonyos-app.html", "harmonyos-probe.html"):
        assert f"resources/rawfile/web/{entry}" in archive.namelist(), f"Packaged {entry} missing"
    assert archive.read("resources/base/media/app_icon.png") == (desktop / "icons/icon.png").read_bytes(), "Packaged icon differs from upstream"
    assert "resources/base/media/app_icon.svg" not in archive.namelist(), "Old platform icon is still packaged"
print(f"HAP checked: ARM64, min API20, target API26, 2in1 + tablet, upstream branding ({hap.stat().st_size:,} bytes)")
