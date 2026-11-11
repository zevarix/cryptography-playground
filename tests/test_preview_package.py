"""Release helpers must preserve notice bytes and reject unsafe/incomplete packages."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from tools.preview_notices import runtime_packages, license_files, checked_source_archive
from tools.preview_package import check_binary_paths, write_archive, verify_payload, write_manifest


class PreviewTests(unittest.TestCase):
    def test_runtime_closure_excludes_build_dev_and_proc_macro(self):
        def p(name, kind="lib"):
            return {"id": name, "name": name, "version": "1", "targets": [{"kind": [kind]}]}
        def dep(name, kind=None):
            return {"pkg": name, "dep_kinds": [{"kind": kind}]}
        m = {"packages": [p("app"), p("runtime"), p("build"), p("dev"), p("derive", "proc-macro"), p("child")],
             "resolve": {"root": "app", "nodes": [
                 {"id": "app", "deps": [dep("runtime"), dep("build", "build"), dep("dev", "dev"), dep("derive")]},
                 {"id": "runtime", "deps": [dep("child")]},
                 *[{"id": name, "deps": []} for name in ["build", "dev", "derive", "child"]]]}}
        self.assertEqual([p["name"] for p in runtime_packages(m)], ["child", "runtime"])

    def test_nested_and_supplemental_notices_retain_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src/unicode_tables").mkdir(parents=True)
            for name, data in [("LICENSE", b"MIT\r\n"), ("LICENSE-THIRD-PARTY", b"additional attribution\n"),
                               ("src/unicode_tables/LICENSE-UNICODE", b"Unicode terms\n")]:
                (root / name).write_bytes(data)
            files = license_files(root)
            self.assertEqual([p.relative_to(root).as_posix() for p in files],
                             ["LICENSE", "LICENSE-THIRD-PARTY", "src/unicode_tables/LICENSE-UNICODE"])
            self.assertEqual(files[0].read_bytes(), b"MIT\r\n")

    def test_declared_filename_and_license_directory_are_retained(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "LICENSES").mkdir()
            (root / "LICENSES/MIT.txt").write_text("MIT terms")
            (root / "terms.txt").write_text("declared terms")
            self.assertEqual([p.relative_to(root).as_posix() for p in license_files(root, "terms.txt")],
                             ["LICENSES/MIT.txt", "terms.txt"])

    def test_missing_notice_does_not_invent_a_license(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "No license"):
                license_files(Path(directory))

    def test_source_archive_must_match_locked_checksum(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.crate"
            path.write_bytes(b"corresponding source")
            with self.assertRaisesRegex(ValueError, "checksum"):
                checked_source_archive(path, "0" * 64)

    def test_binary_scan_checks_ascii_and_utf16_and_allows_remapped_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "app"
            for value in [b"/Users/example/source/file.rs", "C:\\Users\\Example\\file.rs".encode("utf-16-le"),
                          b"x" + "C:\\Users\\Example\\file.rs".encode("utf-16-le")]:
                path.write_bytes(value)
                with self.assertRaisesRegex(ValueError, "workstation"):
                    check_binary_paths(path, [])
            path.write_bytes(b"/src/cryptography-playground/src/lib.rs")
            check_binary_paths(path, [])

    def test_manifest_detects_changed_and_extra_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app").write_bytes(b"binary")
            write_manifest(root, {"sourceCommit": "a" * 40})
            verify_payload(root)
            (root / "unexpected").write_text("extra")
            with self.assertRaisesRegex(ValueError, "inventory"):
                verify_payload(root)
            (root / "unexpected").unlink()
            (root / "app").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "hash"):
                verify_payload(root)

    def test_archive_contains_only_relative_payload_and_preserves_mode(self):
        import zipfile
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            stage = base / "preview"
            stage.mkdir()
            executable = stage / "app"
            executable.write_bytes(b"binary")
            executable.chmod(0o755)
            output = base / "preview.zip"
            write_archive(stage, output)
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(archive.namelist(), ["preview/app"])
                if os.name != "nt":
                    self.assertTrue(archive.getinfo("preview/app").external_attr >> 16 & 0o111)
                self.assertIsNone(archive.testzip())
            with self.assertRaises(FileExistsError):
                write_archive(stage, output)


if __name__ == "__main__":
    unittest.main()
