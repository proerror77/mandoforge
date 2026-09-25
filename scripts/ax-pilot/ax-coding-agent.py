#!/usr/bin/env python3
"""Guest-only Codex adapter. Input is data; no shell, credentials or CLI overrides.

A started marker without a terminal result is intentionally unresolved after a
crash. Never automatically rerun a Codex turn or pretend AX restores its memory.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import signal
import sys

ROOT = Path('/workspace')
RESULT = ROOT / 'mandoforge-ax-result.json'


def run_codex(argv, *, input, env, cwd, timeout_seconds=90):
    process = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True, env=env, cwd=cwd,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(input=input, timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        # The npm launcher spawns a native Codex child. Killing only the launcher
        # leaves that child and its stdout pipes alive, so communicate can hang.
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate(timeout=5)
        raise
    return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)


def execute(request, root=ROOT, codex='codex'):
    result_path = root / RESULT.name
    prompt = request['prompt']
    identity = request['identity']
    digest = hashlib.sha256(prompt.encode()).hexdigest()
    if result_path.exists():
        saved = json.loads(result_path.read_text())
        if saved['identity'] != identity or saved['prompt_sha256'] != digest:
            raise RuntimeError('existing result does not belong to this request')
        return saved
    # Atomic, durable at-most-once intent. An incomplete marker fails closed.
    with (root / '.mandoforge-codex-started').open('x') as marker:
        json.dump({'identity': identity, 'prompt_sha256': digest}, marker)
        marker.flush()
        os.fsync(marker.fileno())
    # Separate per-task home; operator supplies only an explicitly scoped model
    # credential to the guest. Never copy host Codex auth or project credentials.
    home = root / '.codex'
    home.mkdir(exist_ok=True)
    child_env = {k: v for k, v in os.environ.items() if k in ('PATH', 'HOME', 'SSL_CERT_FILE', 'OPENAI_API_KEY')}
    child_env['CODEX_HOME'] = str(home)
    try:
        completed = run_codex(
            [codex, 'exec', '--json', '--skip-git-repo-check', '--ephemeral',
             '--sandbox', 'read-only', '-c', 'approval_policy="never"',
             '-c', 'cli_auth_credentials_store="file"', '-'],
            input=prompt, env=child_env, cwd=root,
        )
        events = []
        for line in completed.stdout.splitlines():
            try:
                event = json.loads(line)
                if isinstance(event, dict):
                    events.append(event)
            except ValueError:
                pass
        result = {'identity': identity, 'prompt_sha256': digest,
                  'exit_code': completed.returncode, 'events': events[:1024],
                  'events_truncated': len(events) > 1024 or len(completed.stdout.encode()) > 900000}
        # stderr may contain credentials or provider responses: do not export it.
    except (subprocess.TimeoutExpired, OSError):
        result = {'identity': identity, 'prompt_sha256': digest,
                  'exit_code': 124, 'events': [], 'events_truncated': False, 'failure': 'timeout_or_spawn_failure'}
    temporary = result_path.with_suffix('.tmp')
    with temporary.open('w') as output:
        json.dump(result, output)
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(result_path)
    return result


if __name__ == '__main__':
    execute(json.loads(sys.argv[1]))
