"""Assemble byte-preserving notices for the locked production dependency closure."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

NOTICE_NAME = re.compile(r"^(?:licen[sc]es?|copying|copyright|notice)(?:[._-]|$)", re.I)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def runtime_packages(metadata):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    seen, todo = set(), [root]
    while todo:
        current = todo.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            package = packages[dep["pkg"]]
            if any(k["kind"] is None for k in dep["dep_kinds"]) and not any(
                "proc-macro" in target["kind"] for target in package["targets"]
            ):
                todo.append(dep["pkg"])
    return sorted((packages[i] for i in seen if i != root), key=lambda p: (p["name"], p["version"]))


def license_files(directory, declared=None):
    files = {p for p in directory.rglob("*") if p.is_file() and any(
        NOTICE_NAME.match(part) for part in p.relative_to(directory).parts)}
    if declared:
        explicit = directory / declared
        if not explicit.is_file():
            raise ValueError("Declared license file is missing: " + directory.name)
        files.add(explicit)
    if any(not p.resolve().is_relative_to(directory.resolve()) for p in files):
        raise ValueError("License file escapes its published package.")
    files = sorted(files)
    if not files:
        raise ValueError("No license/notice text found for " + directory.name)
    return files


def checked_source_archive(path, checksum):
    if not path.is_file() or digest(path) != checksum:
        raise ValueError("Source archive checksum does not match the lockfile: " + path.name)
    return path


def checked_native_component(root, base, entry, target):
    """Reject an unverified native target, artifact or retained SDK notice."""
    if target != entry["target"]:
        raise ValueError("No verified native notice mapping for target " + target)
    artifact = base / entry["artifact"]
    if not artifact.is_file() or digest(artifact) != entry["artifactSHA256"]:
        raise ValueError("Native SDK artifact checksum changed: " + entry["name"])
    paths = []
    for notice in entry["files"]:
        path = root / notice["path"]
        if not path.is_file() or digest(path) != notice["sha256"]:
            raise ValueError("Retained native SDK notice changed: " + notice["path"])
        paths.append(path)
    return paths


def assemble(root, metadata, output, source_commit, target):
    output.mkdir(parents=True, exist_ok=False)
    policy = json.loads((root / "packaging/notice-overrides.json").read_text(encoding="utf-8"))
    locked = tomllib.loads((root / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))["package"]
    records = []

    def copy(path, destination):
        destination = output / destination
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
        return destination.relative_to(output).as_posix()

    for package in runtime_packages(metadata):
        name, version = package["name"], package["version"]
        if not package.get("license"):
            raise ValueError("Missing declared license: " + name)
        base = Path(package["manifest_path"]).parent
        override = policy["packages"].get(name + "@" + version)
        prefix = "cargo/" + name + "-" + version + "/"
        files = []
        if override:
            vcs = json.loads((base / ".cargo_vcs_info.json").read_text(encoding="utf-8"))
            if vcs["git"]["sha1"] != override["vcs"] or package["license"] != override["license"]:
                raise ValueError("Pinned notice override no longer matches " + name)
            for entry in override["files"]:
                path = root / entry["path"]
                if digest(path) != entry["sha256"]:
                    raise ValueError("Retained upstream notice changed: " + entry["path"])
                files.append(copy(path, prefix + "upstream/" + path.name))
        else:
            for path in license_files(base, package.get("license_file")):
                files.append(copy(path, prefix + path.relative_to(base).as_posix()))
        record = {"ecosystem": "cargo", "name": name, "version": version,
                  "license": package["license"], "source": "https://crates.io/crates/" + name + "/" + version,
                  "notices": files}
        if "MPL-2.0" in package["license"] or (override and override.get("includeSource")):
            lock = next(p for p in locked if p["name"] == name and p["version"] == version
                        and p.get("source") == package["source"])
            archive = base.parents[2] / "cache" / base.parent.name / (name + "-" + version + ".crate")
            checked_source_archive(archive, lock["checksum"])
            record["sourceArchive"] = copy(archive, "sources/" + archive.name)
            record["sourceSHA256"] = lock["checksum"]
        records.append(record)
        for native in (override or {}).get("native", []):
            paths = checked_native_component(root, base, native, target)
            records.append({key: native[key] for key in [
                "name", "version", "license", "source", "sourceSHA256",
                "artifact", "artifactSHA256", "target"]} | {
                "ecosystem": "native",
                "notices": [copy(path, "native/" + native["name"] + "/" + path.name) for path in paths]})

    npm_lock = json.loads((root / "package-lock.json").read_text(encoding="utf-8"))
    for relative, package in sorted(npm_lock["packages"].items()):
        if not relative or package.get("dev"):
            continue
        base = root / relative
        actual = json.loads((base / "package.json").read_text(encoding="utf-8"))
        if actual["version"] != package["version"] or not package.get("license"):
            raise ValueError("Installed npm dependency does not match lockfile: " + relative)
        name = actual["name"]
        prefix = "npm/" + name.replace("/", "-") + "-" + actual["version"] + "/"
        records.append({"ecosystem": "npm", "name": name, "version": actual["version"],
                        "license": package["license"], "source": "https://www.npmjs.com/package/" + name,
                        "notices": [copy(p, prefix + p.relative_to(base).as_posix()) for p in license_files(base)]})

    project_files = [
        "LICENSE", "THIRD_PARTY.md", "docs/theme-attributions.md", "docs/theme-license-notices.md",
        "src-tauri/data/LICENSE-cmudict", "src-tauri/data/NOTICE-english-frequency.md",
        "src-tauri/data/wordfreq-notice.txt", "src-tauri/data/english-frequency.txt",
        "src-tauri/data/PROVENANCE.md", "licenses/ciphers-0.1.0-MIT.txt",
        "packaging/UPSTREAM-NOTICES.md",
    ]
    for relative in project_files:
        copy(root / relative, "project/" + relative)
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], cwd=root, text=True).strip())
    rust_notice = sysroot / "share/doc/rust/COPYRIGHT-library.html"
    if not rust_notice.is_file():
        raise ValueError("Install the matching rust-docs component to retain standard-library notices.")
    rust_path = copy(rust_notice, "rust/COPYRIGHT-library.html")
    rust_version = subprocess.check_output(["rustc", "--version"], cwd=root, text=True).strip()
    index = {"sourceCommit": source_commit, "rust": {"version": rust_version, "notice": rust_path,
             "sha256": digest(rust_notice)}, "components": records,
             "scope": "Target-filtered normal dependency closure, excluding proc macros; Cargo feature unification may conservatively include extra components."}
    (output / "index.json").write_text(json.dumps(index, indent=2, sort_keys=True) + "\n")
    lines = ["Third-party components", "", "Source commit: " + source_commit,
             "Project/data/palette notices: project/THIRD_PARTY.md",
             "Rust standard-library copyright and full licenses: rust/COPYRIGHT-library.html",
             "Upstream notice qualifications: project/packaging/UPSTREAM-NOTICES.md", "",
             "Selected MPL components remain under MPL-2.0. Their complete unchanged published",
             "source archives are included below. The project's original code remains MIT.", ""]
    for record in records:
        lines += [record["ecosystem"] + ": " + record["name"] + " " + record["version"],
                  "Declared terms: " + record["license"], "Source: " + record["source"],
                  *["Notice: " + path for path in record["notices"]]]
        if "sourceArchive" in record:
            lines += ["Corresponding source: " + record["sourceArchive"],
                      "Source SHA-256: " + record["sourceSHA256"]]
        lines += [""]
    (output / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(lines), encoding="utf-8")
    return index
