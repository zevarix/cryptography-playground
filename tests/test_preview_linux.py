"""Linux preview validation checks the extracted package that people install."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
from tools import preview_linux
from tools.preview_linux import inspect_payload


class LinuxPreviewTests(unittest.TestCase):
    def fixture(self, root):
        binary = root / "usr/bin/cryptography-playground"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"\x7fELF\x02\x01\x01" + b"\x00" * 9 +
                           struct.pack("<HH", 3, 62) + b"\x00" * 44)
        notices = root / "usr/lib/Cryptography Playground/notices"
        notices.mkdir(parents=True)
        (notices / "LICENSE").write_text("fixture notice")
        (notices / "source.crate").write_bytes(b"fixture source")
        index = {"sourceCommit": "a" * 40, "rust": {"notice": "LICENSE"},
                 "components": [{"notices": ["LICENSE"], "sourceArchive": "source.crate"}]}
        (notices / "index.json").write_text(json.dumps(index))
        return binary, notices

    def test_debian_control_version_sorts_preview_before_stable(self):
        with tempfile.TemporaryDirectory() as directory:
            control = Path(directory) / 'control'
            control.write_text('Package: cryptography-playground\nVersion: 0.1.0-preview.1\nArchitecture: amd64\n')
            preview_linux.normalize_control(control, '0.1.0-preview.1')
            self.assertEqual(control.read_text(), 'Package: cryptography-playground\nVersion: 0.1.0~preview.1\nArchitecture: amd64\n')

    def test_changed_debian_metadata_or_extra_root_file_fails_before_extraction(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'package.deb'
            manifest = root / 'package.deb.manifest.json'
            archive.write_bytes(b'original-debian-archive')
            manifest.write_text(json.dumps({'archiveSHA256': hashlib.sha256(archive.read_bytes()).hexdigest()}))
            for changed in [b'archive-with-mode-change', b'archive-with-extra-root-manifest']:
                archive.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, 'archive.*hash|archive.*checksum'):
                    preview_linux.verify(archive, manifest)

    def test_package_keeps_matching_source_notices_and_native_architecture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary, _ = self.fixture(root)
            result = inspect_payload(root, "a" * 40, "x86_64-unknown-linux-gnu")
            self.assertEqual(result["binary"], binary)
            self.assertEqual(len(result["index"]["components"]), 1)

    def test_wrong_elf_architecture_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary, _ = self.fixture(root)
            data = bytearray(binary.read_bytes())
            struct.pack_into("<H", data, 18, 183)
            binary.write_bytes(data)
            with self.assertRaisesRegex(ValueError, "architecture"):
                inspect_payload(root, "a" * 40, "x86_64-unknown-linux-gnu")

    def test_missing_notice_or_corresponding_source_is_rejected(self):
        for missing in ["LICENSE", "source.crate"]:
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                _, notices = self.fixture(root)
                (notices / missing).unlink()
                with self.assertRaisesRegex(ValueError, "notice|source"):
                    inspect_payload(root, "a" * 40, "x86_64-unknown-linux-gnu")

    def test_notice_source_revision_must_match_the_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            with self.assertRaisesRegex(ValueError, "revision"):
                inspect_payload(root, "b" * 40, "x86_64-unknown-linux-gnu")

    def test_unreviewed_bundled_system_library_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            (root / "usr/lib/libwebkit.so.1").write_bytes(b"unexpected shared library")
            with self.assertRaisesRegex(ValueError, "shared librar"):
                inspect_payload(root, "a" * 40, "x86_64-unknown-linux-gnu")

    def test_notice_path_cannot_escape_the_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, notices = self.fixture(root)
            index = json.loads((notices / "index.json").read_text())
            index["components"][0]["notices"] = ["../../../../outside"]
            (notices / "index.json").write_text(json.dumps(index))
            with self.assertRaisesRegex(ValueError, "notice"):
                inspect_payload(root, "a" * 40, "x86_64-unknown-linux-gnu")


if __name__ == "__main__":
    unittest.main()
