"""Generated ZIP fixtures for the unsigned release export boundary."""
import importlib.util
import io
from pathlib import Path
import struct
import tempfile
import unittest
from zipfile import ZipFile

spec = importlib.util.spec_from_file_location("check_hap", Path(__file__).with_name("check-hap.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


def zip_bytes(name="module.json"):
    output = io.BytesIO()
    with ZipFile(output, "w") as archive:
        archive.writestr(name, "generated fixture")
    return output.getvalue()


class UnsignedHapTests(unittest.TestCase):
    def validate(self, contents):
        with tempfile.TemporaryDirectory(prefix="hatoba-hap-test-") as directory:
            path = Path(directory) / "generated.hap"
            path.write_bytes(contents)
            with ZipFile(path) as archive:
                checker.validate_unsigned(path, archive)

    def test_plain_zip_has_no_signing_block(self):
        self.validate(zip_bytes())

    def test_signing_block_before_central_directory_is_rejected(self):
        original = zip_bytes()
        with ZipFile(io.BytesIO(original)) as archive:
            offset = archive.start_dir
        block = b"generated signing block"
        changed = bytearray(original[:offset] + block + original[offset:])
        struct.pack_into("<I", changed, len(changed) - 6, offset + len(block))
        with self.assertRaisesRegex(AssertionError, "signing block"):
            self.validate(changed)

    def test_appended_data_is_rejected(self):
        with self.assertRaisesRegex(AssertionError, "ZIP trailer"):
            self.validate(zip_bytes() + b"generated trailer")

    def test_prefixed_data_is_rejected(self):
        with self.assertRaisesRegex(AssertionError, "before ZIP entry"):
            self.validate(b"generated prefix" + zip_bytes())

    def test_packaged_signing_material_is_rejected(self):
        with self.assertRaisesRegex(AssertionError, "Signing material"):
            self.validate(zip_bytes("generated-only.p12"))


if __name__ == "__main__":
    unittest.main()
