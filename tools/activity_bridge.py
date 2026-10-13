#!/usr/bin/env python3
"""Project only aggregate activity from the qualified Telecodex provider.

Owner-local adapter config is private. Reuses receiving authorization and native
metadata helpers; never starts turns, consumes bot updates or invokes Stop.
"""
import argparse
import hashlib
import importlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time
import tomllib


def summary(snapshot):
    if snapshot.get('schema') != 'telecodex.sessions-dashboard/v1':
        raise ValueError('unsupported receiving projection')
    sessions = snapshot.get('sessions', [])
    active = sum(s.get('status') == 'active' for s in sessions)
    unknown = sum(s.get('status') not in ('active', 'idle', 'notLoaded') for s in sessions)
    text = f'{active} conversaciones activas'
    if unknown:
        text += f' · {unknown} sin confirmar'
    return {'at': int(snapshot['observed_at']), 'summary': text,
            'source': 'telecodex.sessions-dashboard/v1', 'active': active,
            'unconfirmed': unknown}


def load(config_path):
    path = Path(config_path)
    meta = path.stat()
    if meta.st_uid != os.getuid() or meta.st_mode & 0o077:
        raise ValueError('adapter config must be owner-only')
    config = json.loads(path.read_text())
    source = Path(config['provider_source']).resolve()
    module = source / 'dashboard/provider.py'
    if hashlib.sha256(module.read_bytes()).hexdigest() != config['provider_sha256']:
        raise ValueError('receiving provider changed; requalify before using')
    sys.path.insert(0, str(source))
    provider_module = importlib.import_module('dashboard.provider')
    dashboard_config = tomllib.loads(Path(config['dashboard_config']).read_text())
    provider = provider_module.Provider(dashboard_config)
    if not provider.authorized(provider.human):
        raise ValueError('existing receiving owner grant does not authorize reads')
    return config, provider


def deliver(config, value):
    # Private destination is selected by the operator, never by remote status data.
    command = ['ssh', '-i', config['ssh_key'], '-o', 'IdentitiesOnly=yes',
               '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=4',
               config['ssh_target'], 'python3 -c ' +
               "\"import os,sys;from pathlib import Path;os.umask(0o077);" +
               "p=Path('/run/user/'+str(os.getuid())+'/daimon-integrators/activity.json');" +
               "p.parent.mkdir(mode=0o700,exist_ok=True);" +
               "t=p.with_suffix('.new');t.write_bytes(sys.stdin.buffer.read(4096));t.replace(p)\""]
    subprocess.run(command, input=json.dumps(value).encode(),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                   timeout=8, check=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--config', required=True)
    parser.add_argument('--once', action='store_true')
    args = parser.parse_args()
    config, provider = load(args.config)
    try:
        while True:
            try:
                if not provider.authorized(provider.human):
                    raise ValueError('receiving grant revoked')
                deliver(config, summary(provider.snapshot()))
            except Exception:
                # Old receiver data expires locally; errors do not become idle.
                print('Activity projection unavailable; local device observation continues',
                      file=sys.stderr)
            if args.once:
                break
            time.sleep(10)
    finally:
        provider.close()


if __name__ == '__main__':
    main()
