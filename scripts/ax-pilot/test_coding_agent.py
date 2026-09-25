import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('guest', Path(__file__).with_name('ax-coding-agent.py'))
guest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest)


class GuestTests(unittest.TestCase):
    def test_codex_input_policy_and_no_duplicate_turn(self):
        request = {'identity': {'nonce': 'one'}, 'prompt': 'Explain $(touch /tmp/unsafe) literally'}
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            response = subprocess.CompletedProcess([], 0, '{"type":"turn.completed"}\n', 'sensitive')
            with patch.object(guest, 'run_codex', return_value=response) as run:
                result = guest.execute(request, root)
                args, kwargs = run.call_args
                self.assertEqual(args[0][0:3], ['codex', 'exec', '--json'])
                self.assertIn('read-only', args[0])
                self.assertIn('approval_policy="never"', args[0])
                self.assertEqual(kwargs['input'], request['prompt'])
                self.assertNotIn('shell', kwargs)
                self.assertNotIn('sensitive', json.dumps(result))
                self.assertEqual(guest.execute(request, root), result)
                self.assertEqual(run.call_count, 1)
                with self.assertRaises(RuntimeError):
                    guest.execute({'identity': {'nonce': 'other'}, 'prompt': request['prompt']}, root)

    def test_interrupted_turn_never_restarts(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / '.mandoforge-codex-started').touch()
            with patch.object(guest, 'run_codex') as run:
                with self.assertRaises(FileExistsError):
                    guest.execute({'identity': {}, 'prompt': 'hello'}, root)
                run.assert_not_called()

    def test_timeout_kills_launcher_and_native_process_group(self):
        from unittest.mock import MagicMock
        process = MagicMock(pid=1234)
        process.communicate.side_effect = [subprocess.TimeoutExpired('codex', 90), ('', '')]
        with patch.object(guest.subprocess, 'Popen', return_value=process) as spawn, patch.object(guest.os, 'killpg') as kill:
            with self.assertRaises(subprocess.TimeoutExpired):
                guest.run_codex(['codex'], input='prompt', env={}, cwd='/workspace')
            self.assertTrue(spawn.call_args.kwargs['start_new_session'])
            kill.assert_called_once_with(1234, guest.signal.SIGKILL)

    def test_real_process_tree_timeout_does_not_leave_open_pipes(self):
        import time
        started = time.monotonic()
        with self.assertRaises(subprocess.TimeoutExpired):
            guest.run_codex(['/bin/sh', '-c', 'sleep 30 & wait'], input='', env={}, cwd='/tmp', timeout_seconds=0.1)
        self.assertLess(time.monotonic() - started, 5)

    def test_timeout_is_failure_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            with patch.object(guest, 'run_codex', side_effect=subprocess.TimeoutExpired('codex', 90)):
                result = guest.execute({'identity': {}, 'prompt': 'hello'}, Path(temp))
                self.assertEqual(result['exit_code'], 124)
                self.assertEqual(result['events'], [])


if __name__ == '__main__':
    unittest.main()
