import json
import subprocess
import sys
from pathlib import Path

import yaml
import pytest

ROOT = Path(__file__).resolve().parents[2]


def run_gate(tmp_path, request):
    documents = yaml.safe_load_all((ROOT / "manifests/contribute.yaml").read_text())
    data = next(d["data"] for d in documents if d and d.get("kind") == "ConfigMap")
    assert "verify.py" in data, "required-regression gate asset is not implemented"
    script = tmp_path / "verify.py"
    script.write_text(data["verify.py"])
    return subprocess.run([sys.executable, str(script), json.dumps(request)], capture_output=True, text=True)


def make_tests(tmp_path, required="pass", optional="skip"):
    bodies = {"pass": "self.assertEqual(2 + 2, 4)", "skip": "self.skipTest('optional dependency')", "fail": "self.fail('consumer regression')"}
    (tmp_path / "test_cases.py").write_text(
        "import unittest\nclass Cases(unittest.TestCase):\n"
        f"    def test_required(self):\n        {bodies[required]}\n"
        f"    def test_optional(self):\n        {bodies[optional]}\n"
    )
    return {"cwd": str(tmp_path), "runner": "unittest", "tests": ["test_cases.Cases"], "required": ["test_cases.Cases.test_required"], "python": sys.executable}


def test_required_pass_allows_unrelated_skip(tmp_path):
    result = run_gate(tmp_path, make_tests(tmp_path))
    assert result.returncode == 0, result.stderr + result.stdout
    report = json.loads(result.stdout)
    assert "test_cases.Cases.test_required" in report["passed"]
    assert "test_cases.Cases.test_optional" in report["skipped"]


def test_required_skip_fails_gate(tmp_path):
    result = run_gate(tmp_path, make_tests(tmp_path, required="skip"))
    assert result.returncode != 0


def test_missing_required_test_fails_gate(tmp_path):
    request = make_tests(tmp_path)
    request["required"] = ["test_cases.Cases.test_absent"]
    result = run_gate(tmp_path, request)
    assert result.returncode != 0


def test_setup_failure_stops_before_test_side_effect(tmp_path):
    (tmp_path / "test_cases.py").write_text("from pathlib import Path\nPath('test_started').write_text('ran')\n")
    request = {"cwd": str(tmp_path), "runner": "unittest", "required": ["test_cases.Cases.test_required"], "setup": [[sys.executable, "-c", "raise SystemExit(7)"]]}
    result = run_gate(tmp_path, request)
    assert result.returncode != 0
    assert not (tmp_path / "test_started").exists()


@pytest.mark.parametrize("body", ["self.fail('regression')", "raise RuntimeError('broken')"])
def test_unrelated_failure_or_error_fails_gate(tmp_path, body):
    request = make_tests(tmp_path)
    with (tmp_path / "test_cases.py").open("a") as stream:
        stream.write(f"    def test_broken(self):\n        {body}\n")
    result = run_gate(tmp_path, request)
    assert result.returncode != 0
    assert json.loads(result.stdout)["ok"] is False


@pytest.mark.parametrize("change", [
    {"required": []}, {"required": ["x", "x"]}, {"tests": []},
    {"python": ""}, {"setup": [[""]]}, {"runner": "unknown"},
    {"cwd": "."}, {"command": []},
])
def test_invalid_request_has_no_setup_effect(tmp_path, change):
    request = make_tests(tmp_path)
    request["setup"] = [[sys.executable, "-c", "from pathlib import Path; Path('setup_started').touch()"]]
    request.update(change)
    result = run_gate(tmp_path, request)
    assert result.returncode != 0
    assert json.loads(result.stdout)["ok"] is False
    assert not (tmp_path / "setup_started").exists()


def junit_request(tmp_path, xml, exit_code=0):
    return {
        "cwd": str(tmp_path), "runner": "junit", "required": ["Cases.required"],
        "command": [sys.executable, "-c",
                    "import pathlib,sys; print('child progress'); "
                    "pathlib.Path(sys.argv[1]).write_text(sys.argv[2]); "
                    "raise SystemExit(int(sys.argv[3]))",
                    "{report}", xml, str(exit_code)],
    }


def test_junit_required_pass_allows_unrelated_skip(tmp_path):
    request = junit_request(tmp_path, '<testsuite><testcase classname="Cases" name="required"/><testcase name="optional"><skipped/></testcase></testsuite>')
    result = run_gate(tmp_path, request)
    assert result.returncode == 0, result.stdout + result.stderr
    assert json.loads(result.stdout) == {"ok": True, "passed": ["Cases.required"], "skipped": ["optional"]}
    assert "child progress" in result.stderr


@pytest.mark.parametrize("xml", [
    '<testsuite><testcase classname="Cases" name="required"><skipped/></testcase></testsuite>',
    '<testsuite><testcase classname="Cases" name="required" status="notrun"/></testsuite>',
    '<testsuite><testcase classname="Cases" name="required" result="suppressed"/></testsuite>',
    '<testsuite><testcase classname="Cases" name="required" status="skipped"/></testsuite>',
    '<testsuite><testcase name="other"/></testsuite>',
    '<testsuite><testcase classname="Cases" name="required"/><testcase classname="Cases" name="required"/></testsuite>',
    '<testsuite><testcase classname="Cases" name="required"/><testcase name="other"><failure/></testcase></testsuite>',
    '<testsuite><testcase classname="Cases" name="required"><error/></testcase></testsuite>',
    '<testsuite>', '<unexpected/>',
])
def test_invalid_or_nonpassing_junit_fails(tmp_path, xml):
    result = run_gate(tmp_path, junit_request(tmp_path, xml))
    assert result.returncode != 0
    assert json.loads(result.stdout)["ok"] is False


def test_failed_junit_command_rejects_green_report(tmp_path):
    result = run_gate(tmp_path, junit_request(tmp_path, '<testsuite><testcase classname="Cases" name="required"/></testsuite>', 7))
    assert result.returncode != 0


def test_junit_missing_report_cannot_use_stale_artifact(tmp_path):
    (tmp_path / "report.xml").write_text('<testsuite><testcase classname="Cases" name="required"/></testsuite>')
    request = junit_request(tmp_path, "unused")
    request["command"] = [sys.executable, "-c", "pass", "{report}"]
    result = run_gate(tmp_path, request)
    assert result.returncode != 0
    assert json.loads(result.stdout)["ok"] is False


@pytest.mark.parametrize("arguments", [["no-placeholder"], ["{report}", "{report}"]])
def test_invalid_junit_placeholder_prevents_setup(tmp_path, arguments):
    request = junit_request(tmp_path, "unused")
    request["command"] = [sys.executable, "-c", "pass", *arguments]
    request["setup"] = [[sys.executable, "-c", "from pathlib import Path; Path('setup_started').touch()"]]
    result = run_gate(tmp_path, request)
    assert result.returncode != 0
    assert not (tmp_path / "setup_started").exists()


def test_selected_python_and_cwd_are_preserved(tmp_path):
    python = tmp_path / "selected-python"
    python.write_text(f'#!/bin/sh\nexec "{sys.executable}" -I "$@"\n')
    python.chmod(0o755)
    (tmp_path / "test_cases.py").write_text(
        "import pathlib,sys,unittest\nclass Cases(unittest.TestCase):\n"
        "    def test_required(self):\n"
        "        self.assertTrue(sys.flags.isolated)\n"
        "        self.assertEqual(pathlib.Path.cwd(), pathlib.Path(__file__).parent)\n"
        "        self.assertEqual(pathlib.Path('prepared').read_text(), 'ready')\n"
    )
    request = {"cwd": str(tmp_path), "runner": "unittest", "required": ["test_cases.Cases.test_required"],
               "python": str(python), "setup": [[sys.executable, "-c", "from pathlib import Path; Path('prepared').write_text('ready')"]]}
    result = run_gate(tmp_path, request)
    assert result.returncode == 0, result.stdout + result.stderr


def test_junit_embedded_report_is_unique_and_cleaned(tmp_path):
    producer = tmp_path / "producer.py"
    producer.write_text(
        "import pathlib,sys\n"
        "report = pathlib.Path(sys.argv[1].split('=', 1)[1])\n"
        "assert not report.exists()\n"
        "with pathlib.Path('used_reports').open('a') as stream: stream.write(str(report) + '\\n')\n"
        "report.write_text('<testsuite><testcase classname=\"Cases\" name=\"required\"/></testsuite>')\n"
    )
    request = {"cwd": str(tmp_path), "runner": "junit", "required": ["Cases.required"],
               "command": [sys.executable, str(producer), "--junitxml={report}"]}
    for _ in range(2):
        result = run_gate(tmp_path, request)
        assert result.returncode == 0, result.stdout + result.stderr
    reports = (tmp_path / "used_reports").read_text().splitlines()
    assert len(set(reports)) == 2
    assert all(not Path(report).exists() for report in reports)


def test_setup_runs_once_in_order_and_stops_on_failure(tmp_path):
    request = make_tests(tmp_path)
    request["setup"] = [
        [sys.executable, "-c", "from pathlib import Path; Path('order').write_text('first')"],
        [sys.executable, "-c", "from pathlib import Path; assert Path('order').read_text() == 'first'; Path('order').write_text('second'); raise SystemExit(9)"],
        [sys.executable, "-c", "from pathlib import Path; Path('third').touch()"],
    ]
    result = run_gate(tmp_path, request)
    assert result.returncode != 0
    assert (tmp_path / "order").read_text() == "second"
    assert not (tmp_path / "third").exists()


def test_unittest_child_output_does_not_corrupt_summary(tmp_path):
    request = make_tests(tmp_path)
    with (tmp_path / "test_cases.py").open("a") as stream:
        stream.write("    def test_output(self):\n        print('test progress')\n")
    result = run_gate(tmp_path, request)
    assert result.returncode == 0, result.stdout + result.stderr
    assert json.loads(result.stdout)["ok"] is True
    assert "test progress" in result.stderr
