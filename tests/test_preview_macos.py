"""Apple Silicon packaging must preserve executable integrity without a publisher identity."""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import unittest
from tools import preview_package


class PreviewTargetTests(unittest.TestCase):
    def test_native_target_remains_the_default(self):
        for system, host in [("Windows", "x86_64-pc-windows-msvc"),
                             ("Darwin", "x86_64-apple-darwin")]:
            self.assertEqual(preview_package.select_target(host, system, None), host)

    def test_intel_mac_can_select_apple_silicon(self):
        self.assertEqual(preview_package.select_target(
            "x86_64-apple-darwin", "Darwin", "aarch64-apple-darwin"),
            "aarch64-apple-darwin")

    def test_cross_operating_system_and_unknown_targets_are_rejected(self):
        for host, system, requested in [
            ("x86_64-pc-windows-msvc", "Windows", "aarch64-apple-darwin"),
            ("x86_64-apple-darwin", "Darwin", "x86_64-pc-windows-msvc"),
            ("x86_64-apple-darwin", "Darwin", "universal-apple-darwin"),
        ]:
            with self.subTest(requested=requested, system=system):
                with self.assertRaisesRegex(ValueError, "target"):
                    preview_package.select_target(host, system, requested)


@unittest.skipUnless(platform.system() == "Darwin", "Requires the macOS compiler and signature verifier")
class MacosSignatureTests(unittest.TestCase):
    def fixture(self, root):
        release = root / "release"
        release.mkdir()
        compiled = release / "cryptography-playground"
        subprocess.run([
            "xcrun", "clang", "-arch", "arm64", "-mmacosx-version-min=11.0",
            "-Wl,-adhoc_codesign", "-x", "c", "-", "-o", str(compiled),
        ], input="int main(void) { return 0; }\n", text=True, check=True,
            capture_output=True)
        bundle = release / "bundle/macos/Fixture.app"
        binary = bundle / "Contents/MacOS/cryptography-playground"
        binary.parent.mkdir(parents=True)
        shutil.copy2(compiled, binary)
        return compiled, binary, bundle

    def test_arm_linker_signature_is_verified_without_requiring_a_resource_seal(self):
        with tempfile.TemporaryDirectory() as directory:
            _, binary, bundle = self.fixture(Path(directory))
            self.assertEqual(preview_package.verify_preview_signing(binary, bundle, "aarch64-apple-darwin"),
                             "linker-ad-hoc")

    def test_changed_bundle_executable_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            _, binary, bundle = self.fixture(Path(directory))
            binary.write_bytes(binary.read_bytes() + b"changed by bundler")
            with self.assertRaisesRegex(ValueError, "changed"):
                preview_package.verify_preview_signing(binary, bundle, "aarch64-apple-darwin")

    def test_matching_but_corrupted_executables_fail_native_signature_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            compiled, binary, bundle = self.fixture(Path(directory))
            content = bytearray(compiled.read_bytes())
            content[4096] ^= 1
            compiled.write_bytes(content)
            binary.write_bytes(content)
            with self.assertRaises(subprocess.CalledProcessError):
                preview_package.verify_preview_signing(binary, bundle, "aarch64-apple-darwin")


if __name__ == "__main__":
    unittest.main()
