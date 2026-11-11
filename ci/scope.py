"""Run expensive application checks unless all changed paths are known docs."""
from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess

DOC_FILES = {
    "README.md", "AGENTS.md", "CONTRIBUTING.md", "SECURITY.md",
    "THIRD_PARTY.md", "LICENSE", "LICENSE.md", "RECIPE-001.md",
}


# Curated README images; these are not compiled application inputs.
DOC_IMAGES = {
    "docs/showcase/workbench.png", "docs/showcase/recipes.png", "docs/showcase/files.png",
}

def needs_application_checks(paths: list[str] | None) -> bool:
    if paths is None:
        return True
    return any(
        path not in DOC_FILES
        and path not in DOC_IMAGES
        and not (path.startswith("docs/") and path.endswith(".md"))
        and not (path.startswith(".github/ISSUE_TEMPLATE/") and path.endswith((".md", ".yml", ".yaml")))
        for path in paths
    )


def changed_paths(base: str, head: str) -> list[str] | None:
    if not re.fullmatch(r"[0-9a-f]{40}", base) or base == "0" * 40:
        return None
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        return None
    try:
        # Both sides of a rename matter: moving source into docs must not skip it.
        result = subprocess.run(
            ["git", "diff", "--no-renames", "--name-only", "-z", base, head],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        return [path for path in result.stdout.decode("utf-8").split("\0") if path]
    except (subprocess.CalledProcessError, UnicodeDecodeError):
        return None


def main() -> None:
    manual = os.environ.get("CI_EVENT") == "workflow_dispatch"
    paths = changed_paths(os.environ.get("CI_BASE", ""), os.environ.get("CI_HEAD", ""))
    application = manual or needs_application_checks(paths)
    print("Application checks: " + ("required" if application else "docs only"))
    if paths is None:
        print("Changed paths unavailable; running full checks.")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as handle:
            handle.write(f"application={str(application).lower()}\n")


if __name__ == "__main__":
    main()
