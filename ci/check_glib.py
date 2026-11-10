"""Verify that Cargo selects the audited GLib path patch for the native graph."""

import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--format-version", "1", "--locked",
    "--manifest-path", str(root / "src-tauri/Cargo.toml"),
    "--filter-platform", "x86_64-unknown-linux-gnu",
], text=True))
glib = [package for package in metadata["packages"] if package["name"] == "glib"]
expected = (root / "src-tauri/vendor/glib/Cargo.toml").resolve()
if (
    len(glib) != 1
    or glib[0]["source"] is not None
    or Path(glib[0]["manifest_path"]).resolve() != expected
):
    raise SystemExit("Cargo did not select the retained GLib security backport.")
print(f"Native dependency graph uses glib {glib[0]['version']} from {expected}.")
