#!/usr/bin/env python3
"""Check the actual packaged metadata and ELF, not just the source configuration."""
import argparse
import json
from pathlib import Path
import struct
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[2]

def validate_unsigned(hap: Path, archive: ZipFile):
    """An SDK unsigned HAP has contiguous ZIP records and no separate signing block."""
    raw = hap.read_bytes()
    assert not archive.comment and raw[-22:-18] == b"PK\x05\x06", "Unexpected ZIP trailer"
    cursor = 0
    forbidden = {".p12", ".p7b", ".pem", ".cer", ".jks", ".keystore"}
    for entry in sorted(archive.infolist(), key=lambda item: item.header_offset):
        assert Path(entry.filename).suffix.lower() not in forbidden, "Signing material must not be packaged"
        assert entry.header_offset == cursor, "Unexpected data before ZIP entry (possible signing block)"
        header = struct.unpack_from("<IHHHHHIIIHH", raw, cursor)
        assert header[0] == 0x04034B50, "Invalid local ZIP header"
        cursor += 30 + header[-2] + header[-1] + entry.compress_size
        if entry.flag_bits & 8:
            cursor += 16 if raw[cursor:cursor + 4] == b"PK\x07\x08" else 12
    assert cursor == archive.start_dir, "HAP has a signing block before its central directory"
    central_size = struct.unpack_from("<I", raw, len(raw) - 10)[0]
    assert archive.start_dir + central_size == len(raw) - 22, "Unexpected data after ZIP central directory"


def check_hap(hap: Path, require_release=False, require_unsigned=False):
    expected = json.loads((ROOT / "platform/harmonyos/AppScope/app.json5").read_text())["app"]
    upstream = json.loads((ROOT / "platform/harmonyos/upstream.json").read_text())
    version = json.loads((ROOT / "package.json").read_text())["version"]
    assert expected["versionName"] == upstream["version"] == version, "HarmonyOS, upstream baseline and workspace versions differ"
    desktop = ROOT / "apps/desktop/src-tauri"
    product_name = json.loads((desktop / "tauri.conf.json").read_text())["productName"]
    strings = json.loads((ROOT / "platform/harmonyos/AppScope/resources/base/element/string.json").read_text())["string"]
    assert next(item["value"] for item in strings if item["name"] == "app_name") == product_name, "Application name differs from upstream"
    with ZipFile(hap) as archive:
        metadata = json.loads(archive.read("module.json"))
        app = metadata["app"]
        module = metadata["module"]
        if require_release:
            assert app.get("debug") is False and app.get("buildMode") == "release", "Expected a release HAP with debugging disabled"
        if require_unsigned:
            validate_unsigned(hap, archive)
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


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("hap", nargs="?", type=Path,
                        default=ROOT / "platform/harmonyos/entry/build/default/outputs/default/entry-default-unsigned.hap")
    parser.add_argument("--require-release", action="store_true")
    parser.add_argument("--require-unsigned", action="store_true")
    args = parser.parse_args()
    check_hap(args.hap, args.require_release, args.require_unsigned)
