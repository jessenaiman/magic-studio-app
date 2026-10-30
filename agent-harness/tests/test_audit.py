"""Exercise audit's CLI and real Lizard, including its CSV exit-status trap.

Run: PYTHONPATH=agent-harness python -m unittest discover -s agent-harness/tests -v
Requires the installed lizard executable; no tools are installed by the tests.
"""

import csv
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class AuditTests(unittest.TestCase):
    def setUp(self):
        self.assertIsNotNone(shutil.which("lizard"), "Install Lizard to run this suite")
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        # A worktree's .git is a file, not a directory.
        (self.root / ".git").write_text("gitdir: unused\n")
        self.source = self.root / "src" / "example.rs"
        self.source.parent.mkdir()
        self.config = self.root / "code-limits.json"
        self.config.write_text(json.dumps({
            "max_function_ccn": 2,
            "max_function_lines": 100,
            "max_function_cognitive_complexity": 15,
        }))
        self.source.write_text(
            "fn branchy(x: i32) -> i32 {\n"
            "    if x > 0 { return 1; }\n"
            "    if x < 0 { return -1; }\n"
            "    0\n}\n"
        )

    def run_audit(self, *options):
        env = dict(os.environ)
        env["PYTHONPATH"] = str(Path(__file__).resolve().parents[1])
        return subprocess.run(
            [sys.executable, "-m", "cli_anything.magic.magic_cli", *options],
            cwd=self.root, env=env, text=True, capture_output=True, check=False,
        )

    def test_default_reports_complexity_violation_and_nonzero_status(self):
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("Warnings", result.stdout)
        self.assertIn("branchy", result.stdout)
        self.assertIn("CogC", result.stdout)

    def test_csv_remains_parseable_but_does_not_hide_violations(self):
        result = self.run_audit("audit", str(self.source), "--format", "csv")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        rows = list(csv.reader(io.StringIO(result.stdout)))
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][1], "3")
        self.assertEqual(rows[0][7], "branchy")
        self.assertIn("warning:", result.stderr)

    def test_simple_function_passes_in_both_formats(self):
        self.source.write_text("fn simple() -> i32 { 1 }\n")
        for output_format in ("text", "csv"):
            with self.subTest(output_format=output_format):
                result = self.run_audit(
                    "audit", str(self.source), "--format", output_format,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_length_limit_finds_long_straight_line_function(self):
        self.source.write_text("fn long() {\n" + "    work();\n" * 101 + "}\n")
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("length > 100", result.stdout)

    def test_cognitive_limit_finds_nested_function_below_ccn_limit(self):
        self.config.write_text(json.dumps({
            "max_function_ccn": 100,
            "max_function_cognitive_complexity": 2,
        }))
        self.source.write_text("fn nested(x: bool) { if x { if x { work(); } } }\n")
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("cognitive_complexity > 2", result.stdout)

    def test_invalid_optional_limit_is_setup_error(self):
        self.config.write_text('{"max_function_ccn": 15, "max_function_lines": true}')
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("max_function_lines", result.stderr)

    def test_dry_run_does_not_analyze_or_claim_a_pass(self):
        result = self.run_audit("--dry-run", "audit", str(self.source))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertIn("not run", result.stderr)
        self.assertNotIn("exit 0", result.stderr)

    def test_old_ccn_only_config_is_supported(self):
        self.config.write_text('{"max_function_ccn": 2}')
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)

    def test_directory_lizard_check_also_enforces_thresholds(self):
        result = self.run_audit("audit", str(self.root), "--tool", "lizard")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("branchy", result.stdout)

    def test_missing_config_is_not_a_pass(self):
        self.config.unlink()
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("missing repository configuration", result.stderr)

    def test_extracted_directory_uses_repo_limits_without_auditing_siblings(self):
        self.source.write_text("fn simple() -> i32 { 1 }\n")
        (self.root / "unrelated.rs").write_text(
            "fn unrelated(x: bool) { if x { work(); } if x { work(); } }\n"
        )
        result = self.run_audit("audit", str(self.source.parent), "--tool", "lizard")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("simple", result.stdout)
        self.assertNotIn("unrelated", result.stdout)

    def test_rust_facade_audit_includes_extracted_implementation(self):
        self.source.write_text("mod loading;\n")
        modules = self.source.with_suffix("")
        modules.mkdir()
        (modules / "loading.rs").write_text(
            "fn load(x: bool) { if x { work(); } if x { work(); } }\n"
        )
        result = self.run_audit("audit", str(self.source))
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("loading.rs", result.stdout)
        self.assertIn("2 file analyzed", result.stdout)

    def fake_node_tool(self, name):
        tool = self.root / "node_modules" / ".bin" / name
        tool.parent.mkdir(parents=True, exist_ok=True)
        tool.write_text("#!/bin/sh\nexit 0\n")
        tool.chmod(0o755)

    def test_stylelint_in_subdirectory_uses_repository_config(self):
        self.fake_node_tool("stylelint")
        (self.root / "stylelint.config.mjs").write_text("export default {};\n")
        ui = self.root / "ui"
        ui.mkdir()
        result = self.run_audit("--dry-run", "audit", str(ui), "--tool", "stylelint")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"{ui}/**/*.css", result.stderr)

    def test_wallace_measures_our_stylesheets_without_vendor_css(self):
        self.fake_node_tool("wallace")
        ui = self.root / "ui"
        (ui / "vendor").mkdir(parents=True)
        (ui / "style.css").write_text(".a { color: red; }\n")
        (ui / "vendor" / "lib.css").write_text(".b { color: blue; }\n")
        result = self.run_audit("--dry-run", "audit", str(ui), "--tool", "wallace")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("style.css", result.stderr)
        self.assertNotIn("lib.css", result.stderr)

    def test_clippy_rejects_file_scope_before_running_cargo(self):
        result = self.run_audit("audit", str(self.source), "--tool", "clippy")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("Clippy checks crates", result.stderr)


if __name__ == "__main__":
    unittest.main()
