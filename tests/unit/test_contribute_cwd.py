import json
import subprocess
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]


def check_cwd(tmp_path, cwd, tool="bash"):
    documents = yaml.safe_load_all((ROOT / "manifests/contribute.yaml").read_text())
    data = next(d["data"] for d in documents if d and d.get("kind") == "ConfigMap")
    assert "safe-tools.js" in data, "native cwd guard asset is not implemented"
    script = tmp_path / "safe-tools.js"
    script.write_text(data["safe-tools.js"])
    (tmp_path / "package.json").write_text('{"type":"module"}')
    driver = """const {default:factory}=await import(process.argv[1]); let handler;
factory({on:(name,fn)=>{if(name==='tool_call')handler=fn},registerTool:()=>{},zod:{object:x=>x,string:()=>({optional(){return this}}),number:()=>({int(){return this},positive(){return this},optional(){return this}}),array:()=>({optional(){return this}}),enum:()=>({})}});
Promise.resolve(handler(JSON.parse(process.argv[2]))).then(r=>console.log(JSON.stringify(r??null)));
"""
    event = {"toolName": tool, "input": {"command": "true"}}
    if cwd is not None:
        event["input"]["cwd"] = cwd
    result = subprocess.run(["node", "--input-type=module", "-e", driver, script.as_uri(), json.dumps(event)], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


@pytest.mark.parametrize("cwd", [None, "relative/path"])
def test_bash_rejects_missing_or_relative_cwd(tmp_path, cwd):
    assert check_cwd(tmp_path, cwd)["block"] is True


def test_bash_rejects_file_and_missing_directory(tmp_path):
    file = tmp_path / "not_a_directory"
    file.write_text("keep")
    for cwd in [str(file), str(tmp_path / "missing")]:
        assert check_cwd(tmp_path, cwd)["block"] is True
    assert file.read_text() == "keep"


def test_existing_absolute_directory_is_allowed(tmp_path):
    assert check_cwd(tmp_path, str(tmp_path)) is None


def test_other_tools_are_not_subject_to_bash_cwd_policy(tmp_path):
    assert check_cwd(tmp_path, None, tool="read") is None
