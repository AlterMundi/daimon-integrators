"""Foreground companion transport/capture; canonical sessions remain remote-owned."""
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import uuid


class Unavailable(Exception):
    """No automatic resubmission follows a failed observation or transfer."""


def private_bytes(path, bound=65536):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        meta = os.fstat(stream.fileno())
        if not stat.S_ISREG(meta.st_mode) or meta.st_uid != os.getuid() or meta.st_mode & 0o077:
            raise ValueError('Private owner file required')
        data = stream.read(bound + 1)
        if len(data) > bound:
            raise ValueError('Private record exceeds its bound')
        return data


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def directory(path):
    path = Path(path)
    if not path.is_absolute():
        raise ValueError('Absolute private state directory required')
    if not path.parent.exists():
        directory(path.parent)
    path.mkdir(mode=0o700, exist_ok=True)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError('Private owner directory required')
    sync_directory(path.parent)
    return path


def save(path, value):
    path = Path(path)
    directory(path.parent)
    fd, name = tempfile.mkstemp(prefix='.pending-', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, path)
        sync_directory(path.parent)
    finally:
        if os.path.exists(name):
            os.unlink(name)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


class Transport:
    def __init__(self, credential_file):
        self.path = Path(credential_file)
        self.original = private_bytes(self.path)
        device = json.loads(self.original)
        origin = urllib.parse.urlsplit(device['origin'])
        if (device.get('schema') != 'telecodex.companion-device/v1'
                or origin.scheme != 'https' or not origin.hostname or origin.username or origin.password
                or origin.path or origin.query or origin.fragment
                or not re.fullmatch(r'/[A-Za-z0-9_-]+', device['prefix'])
                or not re.fullmatch(r'Companion [A-Za-z0-9_-]{43}', device['authorization'])
                or not re.fullmatch(r'[A-Za-z0-9_-]{43}', device['csrf'])):
            raise ValueError('Unsupported enrolled device credential')
        self.device = device
        self.identity = hashlib.sha256(self.original).hexdigest()
        self.opener = urllib.request.build_opener(NoRedirect)

    def request(self, method, path, body=None, headers=None, bound=512*1024, timeout=20):
        if private_bytes(self.path) != self.original:
            raise Unavailable('Device enrollment changed; retain saved input and reopen explicitly')
        parsed = urllib.parse.urlsplit(path)
        if (parsed.scheme or parsed.netloc or parsed.fragment
                or not (parsed.path == '/api/interaction' or parsed.path.startswith('/api/interaction/'))
                or '%' in parsed.path or '..' in parsed.path or '\\' in parsed.path or any(ord(c) < 32 for c in path)):
            raise ValueError('Select a canonical conversation API path')
        selected = {'Authorization':self.device['authorization'],
            'Origin':self.device['origin'], 'X-Dashboard-CSRF':self.device['csrf']}
        allowed = {'content-type','x-input-id','x-input-binding','x-audio-sha256','x-reply-source','x-reply-revision'}
        for key, value in (headers or {}).items():
            if key.lower() not in allowed or not isinstance(value,str) or '\r' in value or '\n' in value:
                raise ValueError('Unsupported conversation transport header')
            selected[key] = value
        if body is not None and not isinstance(body, bytes):
            body = json.dumps(body).encode()
            selected['Content-Type'] = 'application/json'
        request = urllib.request.Request(self.device['origin'] + self.device['prefix'] + path,
            data=body, headers=selected, method=method)
        try:
            with self.opener.open(request, timeout=timeout) as response:
                data = response.read(bound + 1)
                if len(data) > bound:
                    raise Unavailable('Conversation response exceeds its bound')
                result_headers = {k.lower():v for k,v in response.headers.items()}
            if private_bytes(self.path) != self.original:
                raise Unavailable('Device enrollment changed during transfer')
            return data, result_headers
        except urllib.error.HTTPError as error:
            code = error.code
            error.close()
            raise Unavailable('Conversation access unavailable' if code in (401,403)
                else 'Conversation outcome unconfirmed; inspect the saved input') from None
        except (urllib.error.URLError, http.client.HTTPException, TimeoutError, OSError):
            raise Unavailable('Connection unavailable; saved inputs must be inspected before retry') from None

    def json(self, method, path, body=None, headers=None):
        data, _ = self.request(method, path, body, headers)
        try:
            value = json.loads(data)
        except (ValueError, UnicodeError):
            raise Unavailable('Conversation response was not confirmed') from None
        if not isinstance(value, dict):
            raise Unavailable('Unsupported conversation response')
        return value


class Drafts:
    def __init__(self, root):
        self.root = directory(root)
        self.media = directory(self.root/'captures')

    def record(self, ident):
        if str(uuid.UUID(ident)) != ident:
            raise ValueError('Canonical original UUID required')
        return self.root/(ident+'.json')

    def begin(self, identity, binding, reply=None):
        ident = str(uuid.uuid4())
        doc = {'schema':'daimon-integrators.companion-draft/v1','id':ident,
            'device_identity':identity, 'binding':binding, 'reply':reply,
            'phase':'capture_preparing'}
        save(self.record(ident),doc)
        return doc

    def read(self, ident):
        return json.loads(private_bytes(self.record(ident)))

    def update(self, ident, **fields):
        doc = self.read(ident)
        immutable = {'id','device_identity','binding','reply','schema'}
        if immutable.intersection(fields):
            raise ValueError('Captured identity and reply selection are immutable')
        doc.update(fields)
        save(self.record(ident),doc)
        return doc

    def audio(self, ident):
        self.record(ident)
        return self.media/(ident+'.m4a')

    def finalize(self, ident, snapshot):
        managed = snapshot['managed']
        if (managed.get('activity') != 'recording'
                or managed.get('output') != str(self.audio(ident))
                or managed.get('pid') is not None
                or managed.get('phase') not in ('stopped','completed')):
            raise Unavailable('Wait for this selected capture to finish before sending')
        fd = os.open(self.audio(ident),os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd,'rb') as stream:
            info = os.fstat(stream.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or not 0 < info.st_size <= 20*1024*1024:
                raise Unavailable('Selected recording is not a bounded regular owner file')
            # The panel's recorder writes only inside this private UUID directory.
            os.fchmod(stream.fileno(),0o600)
            os.fsync(stream.fileno())
            digest = hashlib.sha256(stream.read(20*1024*1024+1)).hexdigest()
        sync_directory(self.media)
        return self.update(ident,phase='ready_to_send',audio_sha256=digest)

    def submit(self, transport, ident):
        doc = self.read(ident)
        if doc['device_identity'] != transport.identity:
            raise Unavailable('Saved clip belongs to an earlier enrollment')
        if doc['phase'] not in ('ready_to_send','submission_unconfirmed'):
            raise ValueError('Only an explicitly finalized clip can be sent')
        clip = private_bytes(self.audio(ident),20*1024*1024)
        digest = hashlib.sha256(clip).hexdigest()
        if doc.get('audio_sha256',digest) != digest:
            raise Unavailable('Saved recording changed; it cannot reuse the original identity')
        self.update(ident,phase='submission_unconfirmed',audio_sha256=digest)
        headers = {'Content-Type':'audio/mp4','X-Input-Id':ident,
            'X-Input-Binding':doc['binding'],'X-Audio-Sha256':digest}
        if doc['reply']:
            headers.update({'X-Reply-Source':doc['reply']['source'],'X-Reply-Revision':doc['reply']['revision']})
        # The caller invokes this only on an explicit Send/Retry gesture.
        result = transport.json('POST','/api/interaction/voice',clip,headers)
        if result.get('id') != ident:
            raise Unavailable('Saved recording admission remains unconfirmed')
        self.update(ident,phase='recorded_remote',remote_phase=result['phase'])
        return result

    def inspect(self, transport, ident):
        doc = self.read(ident)
        if doc['device_identity'] != transport.identity:
            raise Unavailable('Saved clip belongs to an earlier enrollment')
        result = transport.json('GET','/api/interaction/inputs/'+ident)
        if result.get('id') != ident:
            raise Unavailable('Recording receipt selection changed')
        self.update(ident,phase='recorded_remote',remote_phase=result['phase'])
        return result


class Capture:
    def __init__(self, binary='/usr/local/bin/daimon-integrators'):
        self.binary = binary

    def command(self, *args):
        try:
            result = subprocess.run([self.binary,*map(str,args)],stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,timeout=6,check=False)
            if len(result.stdout) > 262144:
                raise Unavailable('Local capture response exceeds its bound')
            value = json.loads(result.stdout)
            if result.returncode or value.get('ok') is not True:
                raise Unavailable('Selected local capture is unavailable')
            return value
        except (OSError, ValueError, subprocess.TimeoutExpired):
            raise Unavailable('Local device state is unavailable') from None

    def status(self):
        return self.command('status')['snapshot']

    def start(self, output):
        return self.command('capture','audio',120,output)

    def stop(self, output):
        return self.command('stop-capture',output)
