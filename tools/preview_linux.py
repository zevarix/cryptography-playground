"""Prepare an unsigned Debian preview; never publish or use signing credentials."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import tempfile
from tools.preview_notices import assemble, digest
from tools.preview_package import check_binary_paths, export_source, host_target, payload, write_manifest

TARGETS = {"x86_64-unknown-linux-gnu": (62, "amd64"),
           "aarch64-unknown-linux-gnu": (183, "arm64")}


def inspect_payload(root, source_commit, target, product_name="Cryptography Playground"):
    binary = root / "usr/bin/cryptography-playground"
    data = binary.read_bytes()
    machine = TARGETS.get(target, (None, None))[0]
    if (len(data) < 64 or data[:7] != b"\x7fELF\x02\x01\x01"
            or struct.unpack_from("<H", data, 18)[0] != machine):
        raise ValueError("ELF architecture does not match the native target.")
    notices = root / "usr/lib" / product_name / "notices"
    if not notices.resolve().is_relative_to(root.resolve()):
        raise ValueError("Unsafe packaged notice directory.")
    index = json.loads((notices / "index.json").read_text(encoding="utf-8"))
    if index["sourceCommit"] != source_commit:
        raise ValueError("Notice revision does not match the packaged source revision.")
    paths = [index["rust"]["notice"]]
    for component in index["components"]:
        paths.extend(component["notices"])
        if component.get("sourceArchive"):
            paths.append(component["sourceArchive"])
    for name in paths:
        path = notices / name
        if (Path(name).is_absolute() or ".." in Path(name).parts or not path.is_file()
                or not path.resolve().is_relative_to(notices.resolve())):
            raise ValueError("Packaged notice or corresponding source is missing or unsafe.")
    if any(p.is_file() for p in root.rglob("*.so*")):
        raise ValueError("Unexpected bundled shared library requires its own distribution review.")
    return {"binary": binary, "index": index}


def normalize_control(control, label):
    text = control.read_text(encoding="utf-8")
    original = "Version: " + label
    if original not in text.splitlines():
        raise ValueError("Unexpected Debian preview version in generated control file.")
    control.write_text(text.replace(original + "\n", "Version: " + label.replace("-", "~", 1) + "\n"),
                       encoding="utf-8")


def verify(archive, manifest):
    expected = json.loads(manifest.read_text(encoding="utf-8"))
    if digest(archive) != expected["archiveSHA256"]:
        raise ValueError("Debian archive checksum differs from its manifest hash.")
    with tempfile.TemporaryDirectory(prefix="playground-deb-verify-") as directory:
        root = Path(directory)
        subprocess.run(["dpkg-deb", "--raw-extract", str(archive), str(root)], check=True)
        actual = payload(root)
        if actual != expected["files"]:
            raise ValueError("Debian payload differs from its file/hash manifest.")
        result = inspect_payload(root, expected["sourceCommit"], expected["target"], expected["productName"])
        check_binary_paths(result["binary"], [str(Path.home())])
        if digest(result["binary"]) != expected["binarySHA256"]:
            raise ValueError("Debian executable differs from the manifest.")
    members = subprocess.check_output(["ar", "t", str(archive)], text=True).splitlines()
    if any(name.startswith("_gpg") for name in members):
        raise ValueError("Expected an unsigned Debian package.")


def build(checkout, output, label):
    if platform.system() != "Linux":
        raise ValueError("Create Debian previews on native Linux.")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-preview\.[0-9]+", label):
        raise ValueError("Use a preview label such as 0.1.0-preview.1")
    clean = lambda: not subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=no"], cwd=checkout, text=True).strip()
    if not clean():
        raise ValueError("Commit reviewed source before creating a preview.")
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
    config = json.loads((checkout / "src-tauri/tauri.conf.json").read_text())
    if label.split("-preview.")[0] != config["version"]:
        raise ValueError("Preview label must match the application's base version.")
    target = host_target(checkout)
    if target not in TARGETS:
        raise ValueError("No Debian/ELF mapping for native target " + target)
    environment = os.environ.copy()
    if environment.get("RUSTFLAGS") or environment.get("CARGO_ENCODED_RUSTFLAGS"):
        raise ValueError("Use a clean environment without custom Rust flags.")
    environment.setdefault("CARGO_TARGET_DIR", str(checkout / "src-tauri/target"))
    output.mkdir(parents=True, exist_ok=False)
    root = output / "source"
    export_hash = export_source(checkout, source, root)
    subprocess.run(["npm", "ci"], cwd=root, check=True)
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1",
        "--locked", "--filter-platform", target, "--features", "tauri/custom-protocol"],
        cwd=root, env=environment))
    notices = output / "notices"
    index = assemble(root, metadata, notices, source, target)
    (notices / "README-preview.txt").write_text(
        "Cryptography Playground " + label + " — unsigned Debian preview\n\n"
        "Experimental ciphers for puzzles and learning, not sensitive information.\n"
        "Install using: sudo apt install ./cryptography-playground-" + label
        + "-linux-" + target.split("-")[0] + "-unsigned.deb\n"
        "Ubuntu/Debian system packages supply GTK and WebKit; no shared system libraries are bundled.\n"
        "Notices: /usr/lib/Cryptography Playground/notices/THIRD-PARTY-NOTICES.txt\n"
        "Upstream qualifications: project/packaging/UPSTREAM-NOTICES.md\n"
        "Source: https://github.com/zevarix/cryptography-playground/tree/" + source + "\n"
        "Verify the adjacent SHA-256 before installation. A checksum is not a publisher signature.\n",
        encoding="utf-8")
    pairs = [(Path.home(), "/build/user"),
             (Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")), "/build/cargo"),
             (checkout, "/src/cryptography-playground"), (root, "/src/cryptography-playground")]
    environment["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(
        "--remap-path-prefix=" + str(path) + "=" + replacement for path, replacement in pairs)
    override = output / "tauri-linux-preview.json"
    override.write_text(json.dumps({"version": label, "bundle": {"resources": {
        os.path.relpath(notices, root / "src-tauri"): "notices"}}}))
    directory = Path(metadata["target_directory"]) / target / "release/bundle/deb"
    previous = {p: p.stat().st_mtime_ns for p in directory.glob("*.deb")}
    subprocess.run(["npm", "run", "tauri", "build", "--", "--target", target,
                    "--config", str(override), "--bundles", "deb", "--no-sign", "--", "--locked"],
                   cwd=root, env=environment, check=True)
    candidates = [p for p in directory.glob("*.deb") if p.stat().st_mtime_ns != previous.get(p)]
    if len(candidates) != 1:
        raise ValueError("Expected exactly one freshly built Debian package.")
    built = candidates[0]
    fields = {field: subprocess.check_output(
        ["dpkg-deb", "--field", str(built), field], text=True).strip()
        for field in ["Package", "Version", "Architecture", "Depends"]}
    if fields["Package"] != "cryptography-playground" or fields["Architecture"] != TARGETS[target][1]:
        raise ValueError("Debian package identity does not match the native application.")
    extracted = output / "extracted"
    subprocess.run(["dpkg-deb", "--raw-extract", str(built), str(extracted)], check=True)
    normalize_control(extracted / "DEBIAN/control", label)
    inspected = inspect_payload(extracted, source, target, config["productName"])
    binary = inspected["binary"]
    check_binary_paths(binary, [str(Path.home()), str(checkout), str(root)])
    subprocess.run(["readelf", "-h", str(binary)], check=True)
    libraries = subprocess.check_output(["ldd", str(binary)], text=True)
    if "not found" in libraries:
        raise ValueError("The packaged executable has an unresolved native dependency.")
    (output / "linked-libraries.txt").write_text(libraries)
    if not clean() or subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip() != source:
        raise ValueError("Source changed during the preview build.")
    name = "cryptography-playground-" + label + "-linux-" + target.split("-")[0] + "-unsigned.deb"
    archive = output / name
    subprocess.run(["dpkg-deb", "--build", "--root-owner-group", str(extracted), str(archive)], check=True)
    fields["Version"] = subprocess.check_output(
        ["dpkg-deb", "--field", str(archive), "Version"], text=True).strip()
    if fields["Version"] != label.replace("-", "~", 1):
        raise ValueError("Debian preview version must sort before its stable release.")
    information = {"sourceCommit": source, "sourceExportSHA256": export_hash,
                   "archiveSHA256": digest(archive), "productName": config["productName"],
                   "preview": label, "appVersion": label, "baseVersion": config["version"],
                   "target": target, "signing": "unsigned", "binarySHA256": digest(binary),
                   "rust": index["rust"]["version"], "debian": fields,
                   "lockfiles": {p: digest(root / p) for p in ["src-tauri/Cargo.lock", "package-lock.json"]}}
    write_manifest(extracted, information)
    manifest = archive.with_suffix(".deb.manifest.json")
    (extracted / "MANIFEST.json").rename(manifest)
    archive.with_suffix(".deb.sha256").write_text(digest(archive) + "  " + name + "\n")
    verify(archive, manifest)
    print(json.dumps({"archive": name, "sha256": digest(archive), "sourceCommit": source,
                      "target": target, "components": len(index["components"])}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--label", default="0.1.0-preview.1")
    parser.add_argument("--verify-deb", type=Path)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    if args.verify_deb and args.manifest:
        verify(args.verify_deb, args.manifest)
        print("Debian payload matches its manifest.")
    elif args.output:
        build(Path(__file__).resolve().parents[1], args.output.resolve(), args.label)
    else:
        parser.error("--output or --verify-deb with --manifest is required")


if __name__ == "__main__":
    main()
