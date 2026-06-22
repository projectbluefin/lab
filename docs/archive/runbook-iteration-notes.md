# Runbook Iteration Notes Archive

Historical notes moved out of `RUNBOOK.md` so the runbook can stay focused on
timeless architecture and durable failure modes.

## Iteration 2 Lessons (2026-05-25)

### dogtail 4.16 API changes — root cause + migration

**Root cause of `requireResult` TypeError:**
`findChild(self, predicate, retry=True)` declares no `**kwargs`. The `@logging_class`
decorator in `logging.py` does strict `sig.bind(*args, **kwargs)` before the function
body runs. Any kwarg not in the signature (for example `requireResult`) raises
`TypeError` at the decorator layer, before reaching `find_descendant`.
`find_descendant(**kwargs)` does accept `requireResult` in its allowed kwargs, but
`findChild` never passes unknown kwargs through.

**`retry=True` causes 20-second waits:** Default `findChild(pred)` uses `retry=True`,
which retries about 20 times with 1-second sleep when the node is not found. Use
`retry=False` for presence-check calls to avoid 20-second hangs in tests.

**Migration table:**

```python
# OLD (broken on dogtail 4.16 — TypeError at logging decorator)
node = root.findChild(pred, requireResult=True)   # TypeError
node = root.findChild(pred, requireResult=False)  # TypeError
node = root.findChild(pred)                       # works but 20s wait if missing

# NEW (correct)
# 1. Require node exists (raises SearchError if missing):
node = root.findChild(pred, retry=True)   # same as default — raises if not found

# 2. Fast fail (raises SearchError after 1 attempt, no 20s wait):
node = root.findChild(pred, retry=False)

# 3. No-raise / check-if-present (replaces requireResult=False):
nodes = root.findChildren(pred)
node = nodes[0] if nodes else None

# 4. Boolean presence check:
if root.findChildren(pred):
    ...
```

### qecore `run_and_save` — 5-second timeout rule

- **Output attribute:** `context.command_stdout` (not `context.last_command_output`)
- **Timeout:** 5-second hard limit on the subprocess. Any command that may produce
  large output must be bounded. Pattern for journalctl:

  ```bash
  journalctl --lines=50 -p err..emerg
  ```

  Do not use bare `journalctl -b` — it times out and returns empty output.

### GNOME 50.1 AT-SPI gaps

On Bluefin 44 (GNOME Shell 50.1), the top-bar panel exposes no toggle buttons for the
clock or system status area at any AT-SPI depth. Only `Activities` and `Show Apps`
exist.

Implications:

- `@quick_settings` and `@calendar` scenarios cannot use AT-SPI to open these menus
- Fix requires one of: GNOME Shell `unsafe_mode` eval, coordinate-based click, or
  checking whether `org.gnome.desktop.interface toolkit-accessibility` is enabled
- Enable unsafe mode before AT-SPI interaction:

  ```bash
  gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
    --method org.gnome.Shell.Eval 'global.context.unsafe_mode = true'
  ```

- Tracked in castrojo/testing-lab #5

### Test file delivery (git-sync, not ConfigMap)

Test files are delivered to the runner pod via the `git-sync` initContainer in
`run-gnome-tests`. It clones `castrojo/testing-lab` (depth 1) into `/workspace`
at the start of every run. No ConfigMap sync, no hostPath for test files.

To update tests: edit `tests/<suite>/`, commit, push to `main`.

### Artifact reading

`run-gnome-tests` prints `results.json` to stderr at run end (captured by Loki and
retrievable via Argo MCP `logs_workflow` or `just logs`). Titan VMs retain
`/tmp/results/` between runs — access via a future workflow step or Loki query.

`podGC: OnWorkflowSuccess` deletes pods on success only. On failure, pods linger until
TTL. Use Argo MCP `logs_workflow` to read results without execing into pods.
