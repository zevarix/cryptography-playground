import unittest
from unittest.mock import patch
import subprocess

from scope import changed_paths, needs_application_checks


class ScopeTests(unittest.TestCase):
    def test_known_docs_skip_application_checks(self):
        self.assertFalse(needs_application_checks([
            "README.md", "SECURITY.md", "docs/recipe-format.md",
            ".github/ISSUE_TEMPLATE/bug.yml",
        ]))

    def test_source_data_lockfiles_and_workflow_run_checks(self):
        for path in [
            "src/App.tsx", "src-tauri/src/engine.rs", "recipes/012.json",
            "src-tauri/data/cmudict.dict", "src-tauri/Cargo.lock",
            "package-lock.json", ".github/workflows/validate.yml", "ci/scope.py",
        ]:
            with self.subTest(path=path):
                self.assertTrue(needs_application_checks(["README.md", path]))

    def test_unknown_path_and_unavailable_diff_run_checks(self):
        for path in ["new-input-format/spec.bin", "docs/executable.rs", ".github/ISSUE_TEMPLATE/helper.py"]:
            with self.subTest(path=path):
                self.assertTrue(needs_application_checks([path]))
        self.assertTrue(needs_application_checks(None))

    def test_empty_verified_diff_can_skip_application_checks(self):
        self.assertFalse(needs_application_checks([]))

    def test_rename_from_source_to_docs_runs_checks(self):
        self.assertTrue(needs_application_checks(["src/old.ts", "docs/old.md"]))

    def test_invalid_or_initial_commit_base_is_unknown(self):
        for base in ["", "0" * 40, "main", "a" * 39]:
            with self.subTest(base=base):
                self.assertIsNone(changed_paths(base, "b" * 40))

    def test_git_failure_is_unknown(self):
        with patch("scope.subprocess.run", side_effect=subprocess.CalledProcessError(1, "git")):
            self.assertIsNone(changed_paths("a" * 40, "b" * 40))

    def test_nul_separated_names_are_preserved(self):
        result = subprocess.CompletedProcess([], 0, stdout=b"docs/a file.md\0src/x.ts\0")
        with patch("scope.subprocess.run", return_value=result) as run:
            self.assertEqual(changed_paths("a" * 40, "b" * 40), ["docs/a file.md", "src/x.ts"])
            self.assertIn("--no-renames", run.call_args.args[0])


if __name__ == "__main__":
    unittest.main()
