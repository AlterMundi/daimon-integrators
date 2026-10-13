"""Disposable mount/network namespace qualification of installer file effects.

Run explicitly with root + bubblewrap. No host service/device/access mutation.
"""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

SOURCE=Path(__file__).resolve().parents[1]


def setup(root):
    for directory in ['etc/systemd/system','etc/ssh','etc/daimon-integrators',
                      'usr-local/bin','var','home/tester/.config/systemd/user',
                      'home/tester/.config/autostart','home/tester/.ssh','test-bin']:
        (root/directory).mkdir(parents=True,exist_ok=True)
    (root/'etc/passwd').write_text('root:x:0:0:root:/root:/bin/sh\ntester:x:1000:1000:tester:/home/tester:/bin/sh\n')
    (root/'etc/group').write_text('root:x:0:\ntester:x:1000:\n')
    (root/'etc/nsswitch.conf').write_text('passwd: files\ngroup: files\n')
    (root/'test-bin/systemctl').write_text('''#!/bin/sh
printf '%s\\n' "$*" >> /var/service-calls
case "$*" in
 *is-active*|*is-enabled*) exit 1;;
esac
exit 0
''')
    (root/'test-bin/runuser').write_text('''#!/bin/sh
while [ "$1" != -- ]; do shift; done
shift
exec "$@"
''')
    (root/'test-bin/cp').write_text('''#!/bin/sh
if [ -e /var/fail-copy ]; then exit 77; fi
exec /usr/bin/cp "$@"
''')
    for path in (root/'test-bin').iterdir():path.chmod(0o755)
    (root/'etc/ssh/sshd_config').write_text('ADMIN_SENTINEL\n')
    (root/'home/tester/.ssh/authorized_keys').write_text('ADMIN_KEYS_SENTINEL\n')


def invoke(root,script):
    command=['bwrap','--unshare-pid','--unshare-net','--unshare-uts','--unshare-ipc',
             '--ro-bind','/usr','/usr','--symlink','/usr/lib','/lib',
             '--symlink','/usr/lib64','/lib64','--symlink','/usr/bin','/bin',
             '--symlink','/usr/sbin','/sbin','--proc','/proc','--dev','/dev',
             '--tmpfs','/tmp','--bind',str(root/'usr-local'),'/usr/local',
             '--bind',str(root/'etc'),'/etc','--bind',str(root/'var'),'/var',
             '--bind',str(root/'home'),'/home',
             '--ro-bind',str(root/'test-bin'),'/test-bin',
             '--ro-bind',str(SOURCE),'/src',
             '--setenv','PATH','/test-bin:/usr/bin:/bin',
             '/bin/sh','-c',script]
    return subprocess.run(command,capture_output=True,text=True)


def digest(root):
    paths=['etc/ssh/sshd_config','home/tester/.ssh/authorized_keys']
    return [(p,hashlib.sha256((root/p).read_bytes()).hexdigest()) for p in paths]


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--scratch',required=True)
    args=parser.parse_args();assert os.getuid()==0,'Run with sudo in the disposable namespace fixture'
    scratch=Path(args.scratch);scratch.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='installation-',dir=scratch) as temp:
        root=Path(temp);setup(root);original=digest(root)
        # Existing component files must all survive a failed snapshot + retry + rollback.
        preserved=['usr-local/bin/daimon-integrators','etc/systemd/system/daimon-integrators-observer.service',
                   'etc/daimon-integrators/devices.json','home/tester/.config/systemd/user/daimon-integrators-panel.service',
                   'home/tester/.config/autostart/daimon-integrators.desktop']
        originals={}
        for index,name in enumerate(preserved):
            p=root/name;value='{"camera":"/dev/test","audio":"test"}\n' if name.endswith('json') else f'ORIGINAL-{index}\n'
            p.write_text(value)
            if name.startswith('home/'):os.chown(p,1000,1000);p.chmod(0o600)
            originals[name]=(p.read_bytes(),p.stat().st_uid,p.stat().st_gid,p.stat().st_mode&0o777)
        (root/'var/fail-copy').touch()
        install='/src/tools/install.sh /src/target/release/daimon-integrators /src tester'
        failed=invoke(root,install);assert failed.returncode!=0,'Injected snapshot failure was ignored'
        (root/'var/fail-copy').unlink()
        installed=invoke(root,install);assert installed.returncode==0,installed.stderr
        rolled=invoke(root,'/src/tools/rollback.sh tester');assert rolled.returncode==0,rolled.stderr
        for name,value in originals.items():
            p=root/name
            assert (p.read_bytes(),p.stat().st_uid,p.stat().st_gid,p.stat().st_mode&0o777)==value,f'Incomplete rollback after interrupted snapshot: {name}'
        assert digest(root)==original,'Administrative access files changed'
        # Fresh-install removal and idempotent retry, preserving unrelated access/media.
        for name in preserved:(root/name).unlink()
        # Begin a fresh installation transaction with no preexisting snapshot.
        shutil.rmtree(root/'var/lib/daimon-integrators')
        for _ in range(2):
            r=invoke(root,install);assert r.returncode==0,r.stderr
        r=invoke(root,'/src/tools/rollback.sh tester');assert r.returncode==0,r.stderr
        assert all(not(root/name).exists() for name in preserved)
        assert digest(root)==original
        print('Isolated interrupted-snapshot/retry/rollback and fresh-install removal passed; administrative sentinels unchanged')

if __name__=='__main__':main()
