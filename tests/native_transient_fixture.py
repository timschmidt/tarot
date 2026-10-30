#!/usr/bin/env python3
"""Unprivileged catalog-launched runner for the actual Wayland integration tests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--audit',type=Path,required=True)
    p.add_argument('--case', choices=['lifecycle', 'notifications'], default='lifecycle')
    args=p.parse_args()
    assert os.geteuid()==10000 and args.output.is_dir()
    audit=json.loads(args.audit.read_text())
    digest=hashlib.sha256(args.binary.read_bytes()).hexdigest()
    assert digest==audit['binary_sha256']
    env=dict(os.environ,TAROT_TEST_CONTROL_SOCKET='/run/user/10000/tarot-session/control.sock',
        TAROT_TEST_INPUT_SOCKET=str(args.output/'input.sock'),TAROT_TEST_CAPTURE_DIR=str(args.output))
    result={'passed':False,'runner_pid':os.getpid(),'uid':os.geteuid(),'cgroup':Path('/proc/self/cgroup').read_text(),'binary_sha256':digest}
    def save(name):
        target=args.output/name;temporary=target.with_suffix(target.suffix+'.tmp')
        temporary.write_text(json.dumps(result,indent=2)+'\n');os.replace(temporary,target)
    with (args.output/'protocol-test.log').open('x') as log:
        selection=['notification_well_keeps_oversized_client_input_and_popup_alignment'] if args.case=='notifications' else ['--skip','notification_well_keeps_oversized_client_input_and_popup_alignment']
        result['case']=args.case
        child=subprocess.Popen([str(args.binary),'--ignored','--nocapture','--test-threads=1',*selection],env=env,
            stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT)
        result['fixture_pid']=child.pid
        save('fixture-started.json')
        try:
            result['test_exit']=child.wait(timeout=240)
        finally:
            if child.poll() is None:
                child.terminate()
                try:child.wait(timeout=6)
                except subprocess.TimeoutExpired:child.kill();child.wait()
            result['passed']=child.returncode==0
            result['completed_monotonic']=time.monotonic()
            save('fixture-result.json')
    assert result['passed'],result


if __name__=='__main__':main()
