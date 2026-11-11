"""Build unsigned native preview ZIPs; never publish a release or use signing credentials."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import subprocess
import tarfile
import zipfile
from tools.preview_notices import assemble, digest


def export_source(checkout, revision, destination):
    """Export tracked Git objects, excluding local/untracked/ignored build inputs."""
    destination.mkdir()
    archive = destination.with_name(destination.name + ".tar")
    with archive.open("xb") as stream:
        subprocess.run(["git", "archive", "--format=tar", revision], cwd=checkout, stdout=stream, check=True)
    with tarfile.open(archive) as source:
        source.extractall(destination, filter="data")
    return digest(archive)


def check_binary_paths(path, prefixes):
    data = path.read_bytes()
    texts = [data.decode("latin1"), *[data[offset:].decode("utf-16-le", errors="ignore") for offset in (0, 1)]]
    patterns = [r"/(?:Users|home)/", r"[a-z]:[\\/]Users[\\/]"]
    for text in texts:
        if any(re.search(pattern, text, re.I) for pattern in patterns) or any(
            prefix.casefold() in text.casefold() for prefix in prefixes if prefix
        ):
            raise ValueError("A workstation path remains embedded in the native executable.")


def payload(root):
    entries = {}
    for path in sorted(root.rglob("*")):
        if path.name == "MANIFEST.json" and path.parent == root:
            continue
        if path.is_symlink():
            target = os.readlink(path)
            if os.path.isabs(target) or not path.resolve().is_relative_to(root.resolve()):
                raise ValueError("Archive symlink escapes the package.")
            content = target.encode("utf-8")
            kind = "symlink"
        elif path.is_file():
            content = path.read_bytes()
            kind = "file"
        else:
            continue
        entries[path.relative_to(root).as_posix()] = {
            "sha256": hashlib.sha256(content).hexdigest(), "bytes": len(content), "kind": kind}
    return entries


def write_manifest(root, information):
    manifest = {**information, "files": payload(root)}
    with (root / "MANIFEST.json").open("x", encoding="utf-8") as stream:
        json.dump(manifest, stream, indent=2, sort_keys=True)
        stream.write("\n")
    return manifest


def verify_payload(root):
    expected = json.loads((root / "MANIFEST.json").read_text(encoding="utf-8"))["files"]
    for name in expected:
        path = PurePosixPath(name)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError("Unsafe manifest path")
    actual = payload(root)
    if set(actual) != set(expected):
        raise ValueError("Payload inventory differs from the manifest.")
    if actual != expected:
        raise ValueError("Payload hash differs from the manifest.")


def write_archive(stage, output):
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(stage.rglob("*")):
            if not path.is_file() and not path.is_symlink():
                continue
            info = zipfile.ZipInfo(stage.name + "/" + path.relative_to(stage).as_posix())
            info.create_system = 3
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = path.lstat().st_mode << 16
            archive.writestr(info, os.readlink(path).encode("utf-8") if path.is_symlink() else path.read_bytes())
        if archive.testzip() is not None:
            raise ValueError("Archive CRC verification failed.")


def host_target(root):
    verbose = subprocess.check_output(["rustc", "-vV"], cwd=root, text=True)
    return next(line.removeprefix("host: ") for line in verbose.splitlines() if line.startswith("host: "))


def unsigned(binary, bundle):
    if platform.system() == "Darwin":
        result = subprocess.run(["codesign", "--display", "--verbose=2", str(bundle)], capture_output=True, text=True)
        if result.returncode == 0 or "not signed at all" not in result.stderr:
            raise ValueError("Expected a completely unsigned macOS app bundle.")
    else:
        quoted = str(binary).replace("'", "''")
        script = "$s=Get-AuthenticodeSignature -LiteralPath '" + quoted + "';if($s.Status.ToString() -ne 'NotSigned'){exit 1}"
        encoded = base64.b64encode(script.encode("utf-16-le")).decode("ascii")
        subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded], check=True)


def build(root, output, label):
    if platform.system() not in {"Darwin", "Windows"}:
        raise ValueError("Use native Windows/macOS for these preview ZIPs; Linux needs its own distribution bundle.")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-preview\.[0-9]+", label):
        raise ValueError("Use a preview label such as 0.1.0-preview.1")
    if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True).strip():
        raise ValueError("Commit the reviewed source before creating preview packages.")
    source = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    app_config = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    if label.split("-preview.")[0] != app_config["version"]:
        raise ValueError("Preview label must match the application version.")
    target = host_target(root)
    system = "macos" if platform.system() == "Darwin" else "windows"
    name = "cryptography-playground-" + label + "-" + system + "-" + target.split("-")[0] + "-unsigned"
    output.mkdir(parents=True, exist_ok=False)
    checkout = root
    root = output / "source"
    exported_hash = export_source(checkout, source, root)
    environment = os.environ.copy()
    if environment.get("RUSTFLAGS") or environment.get("CARGO_ENCODED_RUSTFLAGS"):
        raise ValueError("Use a clean build environment without pre-existing Rust flags.")
    environment.setdefault("CARGO_TARGET_DIR", str(checkout / "src-tauri/target"))
    npm = shutil.which("npm.cmd" if os.name == "nt" else "npm")
    if not npm:
        raise ValueError("npm is required.")
    subprocess.run([npm, "ci"], cwd=root, check=True)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--manifest-path", "src-tauri/Cargo.toml", "--format-version", "1",
         "--locked", "--filter-platform", target, "--features", "tauri/custom-protocol"], cwd=root, env=environment))
    notices = output / "notices"
    index = assemble(root, metadata, notices, source)
    pairs = [(Path.home(), "/build/user"),
             (Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")), "/build/cargo"),
             (checkout, "/src/cryptography-playground")]
    if not root.is_relative_to(checkout):
        pairs.append((root, "/src/cryptography-playground"))
    flags = []
    for path, replacement in pairs:
        for prefix in dict.fromkeys([str(path), path.as_posix()]):
            flags.append("--remap-path-prefix=" + prefix + "=" + replacement)
    environment["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    override = output / "tauri-preview.json"
    override.write_text(json.dumps({"bundle": {"resources": {
        os.path.relpath(notices, root / "src-tauri").replace("\\", "/"): "notices"
    }}}))
    command = [npm, "run", "tauri", "build", "--", "--target", target, "--config", str(override), "--no-sign"]
    command += ["--bundles", "app"] if system == "macos" else ["--no-bundle"]
    subprocess.run([*command, "--", "--locked"], cwd=root, env=environment, check=True)
    target_root = Path(metadata["target_directory"])
    release = target_root / target / "release"
    stage = output / name
    stage.mkdir()
    if system == "macos":
        bundle = release / "bundle/macos/Cryptography Playground.app"
        binary = bundle / "Contents/MacOS/cryptography-playground"
        if not (bundle / "Contents/Resources/notices/index.json").is_file():
            raise ValueError("The macOS app is missing its bundled notice inventory.")
        destination = stage / bundle.name
        shutil.copytree(bundle, destination, symlinks=True)
        bundled_notice = "Cryptography Playground.app/Contents/Resources/notices"
    else:
        bundle = None
        binary = release / "cryptography-playground.exe"
        shutil.copy2(binary, stage / binary.name)
        shutil.copytree(notices, stage / "notices")
        bundled_notice = "notices"
    check_binary_paths(binary, [str(Path.home()), str(checkout), checkout.as_posix(), str(root), root.as_posix()])
    unsigned(binary, bundle)
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip() != source or subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=no"], cwd=checkout, text=True).strip():
        raise ValueError("Source changed during the preview build.")
    shutil.copyfile(root / "LICENSE", stage / "LICENSE")
    readme = (
        "Cryptography Playground " + label + " — unsigned preview\n\n"
        "These experimental and historical ciphers are for puzzles and learning, not sensitive information.\n"
        "Platform: " + target + "\n"
        "Source: https://github.com/zevarix/cryptography-playground/tree/" + source + "\n\n"
        + ("Extract the entire folder and open cryptography-playground.exe. Keep notices alongside it.\n"
           "Requires Microsoft Edge WebView2 Runtime: https://developer.microsoft.com/microsoft-edge/webview2/\n"
           if system == "windows" else
           "Extract the archive, then move Cryptography Playground.app to Applications if desired.\n"
           "This Intel app is unsigned and unnotarized. Apple's per-app opening guidance:\n"
           "https://support.apple.com/en-us/102445\n")
        + "\nNotices and corresponding source: " + bundled_notice + "/THIRD-PARTY-NOTICES.txt\n"
        "Upstream notice qualifications: " + bundled_notice + "/project/packaging/UPSTREAM-NOTICES.md\n"
        "Verify archive SHA-256 with its .sha256 file. Checksums detect corruption, not publisher identity.\n"
    )
    (stage / "README.txt").write_text(readme, encoding="utf-8")
    information = {"sourceCommit": source, "sourceExportSHA256": exported_hash, "preview": label, "appVersion": app_config["version"],
                   "target": target, "signing": "unsigned", "binarySHA256": digest(binary),
                   "rust": index["rust"]["version"],
                   "node": subprocess.check_output(["node", "--version"], cwd=root, text=True).strip(),
                   "lockfiles": {p: digest(root / p) for p in ["src-tauri/Cargo.lock", "package-lock.json"]}}
    write_manifest(stage, information)
    verify_payload(stage)
    archive = output / (name + ".zip")
    write_archive(stage, archive)
    with archive.with_suffix(".zip.sha256").open("x", encoding="utf-8") as stream:
        stream.write(digest(archive) + "  " + archive.name + "\n")
    print(json.dumps({"archive": archive.name, "sha256": digest(archive), "sourceCommit": source,
                      "target": target, "components": len(index["components"])}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--label", default="0.1.0-preview.1")
    parser.add_argument("--verify", type=Path)
    args = parser.parse_args()
    if args.verify:
        verify_payload(args.verify)
        print("Payload matches its manifest.")
    elif args.output:
        build(Path(__file__).resolve().parents[1], args.output.resolve(), args.label)
    else:
        parser.error("--output or --verify is required")


if __name__ == "__main__":
    main()
