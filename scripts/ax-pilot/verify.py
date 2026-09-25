#!/usr/bin/env python3
"""Live local-only API -> approval -> AX -> evidence readback; never a mock gate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import time
import urllib.request
from urllib.parse import urlsplit
import uuid

parser = argparse.ArgumentParser()
parser.add_argument('--base-url', default='http://127.0.0.1:18787')
parser.add_argument('--output', required=True)
parser.add_argument('--prompt', help='Use the real Codex guest instead of the diagnostic')
args = parser.parse_args()
assert urlsplit(args.base_url).hostname in ('127.0.0.1', 'localhost', '::1'), 'local pilot only'
out = Path(args.output)
out.mkdir(parents=True, exist_ok=False)


def api(path, data=None):
    request = urllib.request.Request(args.base_url + path,
        data=None if data is None else json.dumps(data).encode(),
        headers={'Content-Type': 'application/json', 'x-mandoforge-subject': 'admin-1', 'x-mandoforge-roles': 'admin'})
    with urllib.request.urlopen(request, timeout=240) as response:
        return json.load(response)


def save(name, data):
    (out / (name + '.json')).write_text(json.dumps(data, indent=2))


env = {name: os.environ[name] for name in ('MANDOFORGE_AX_CLI', 'MANDOFORGE_AX_SERVER', 'MANDOFORGE_AX_CONTEXT', 'MANDOFORGE_AX_IMAGE')}
env['MANDOFORGE_AX_PILOT_ENABLED'] = '1'
env['MANDOFORGE_AX_CLI_SHA256'] = hashlib.sha256(Path(env['MANDOFORGE_AX_CLI']).read_bytes()).hexdigest()
profile_name = 'ax-pilot-' + uuid.uuid4().hex[:12]
profile = api('/api/agent-runtime-profiles', {'name': profile_name, 'runtime_type': 'ax_pilot',
    'command': os.environ['MANDOFORGE_AX_PILOT_BIN'], 'default_args': [], 'env': env,
    'timeout_seconds': 240, 'remote_computer_required': False})
agent = api('/api/agents', {'name': profile_name, 'kind': 'specialist', 'agent_role': 'specialist',
    'provider': 'openai-compatible', 'model': 'gpt-5.5-mini', 'runtime_profile_id': profile['id'], 'tools': ['agent_cli.exec']})
session = api('/api/sessions', {'agent_id': agent['id'], 'title': 'Isolated AX pilot readback'})
key = str(uuid.uuid4())
task = {'operation': 'run', 'key': key}
if args.prompt:
    task['prompt'] = args.prompt
submission = api('/api/tools/agent_cli.exec/execute', {'session_id': session['id'],
    'args': {'profile': profile_name, 'task': json.dumps(task), 'args': []}})
save('submission', submission)
assert submission.get('approval_id'), 'AX execution must require approval'
# Before approving, there must be no runtime event: this is the governance check.
before = api('/api/sessions/' + session['id'] + '/events')
assert not any(e['event_type'] == 'agent_cli.task.started' for e in before)
save('before-approval', before)
try:
    decision = api('/api/approvals/' + submission['approval_id'] + '/approve', {})
    save('approval', decision)
except Exception as error:
    save('approval-error', {'error': str(error)})
for _ in range(30):
    calls = api('/api/sessions/' + session['id'] + '/tool-calls')
    if any(c['status'] in ('completed', 'failed') for c in calls):
        break
    time.sleep(1)
events = api('/api/sessions/' + session['id'] + '/events')
artifacts = api('/api/sessions/' + session['id'] + '/artifacts')
save('tool-calls', calls)
save('events', events)
save('artifacts', artifacts)
audit = api('/api/sessions/' + session['id'] + '/audit-logs')
save('audit', audit)
save('identity', {'profile_id': profile['id'], 'profile_name': profile_name, 'agent_id': agent['id'], 'session_id': session['id'], 'key': key})
success = (
    any(c['status'] == 'completed' and c.get('result', {}).get('runtime_type') == 'ax_pilot' for c in calls)
    and any(e['event_type'] == 'runtime.final' and any(a['id'] == e['payload'].get('artifact_id') for a in artifacts) for e in events)
    and any(a['action'] == 'tool.completed' and a.get('details', {}).get('runtime_type') == 'ax_pilot' for a in audit)
)
summary = {'evidence_class': 'local_pilot', 'real_ax': True, 'mode': 'codex' if args.prompt else 'diagnostic',
           'verified': success, 'session_id': session['id'], 'key': key,
           'approval_id': submission['approval_id'], 'event_count': len(events), 'artifact_count': len(artifacts)}
save('summary', summary)
print(json.dumps(summary))
raise SystemExit(0 if success else 1)
