"""Foreground companion transport/capture; canonical sessions remain remote-owned."""
import argparse
import asyncio
import fcntl
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


class Unavailable(Exception):
    """No automatic resubmission follows a failed observation or transfer."""
    def __init__(self, message, status=None):
        super().__init__(message)
        self.status = status


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
                else 'Conversation outcome unconfirmed; inspect the saved input', status=code) from None
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
        self.lock = threading.RLock()

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
        with self.lock:
            doc = self.read(ident)
            immutable = {'id','device_identity','binding','reply','schema'}
            if immutable.intersection(fields):
                raise ValueError('Captured identity and reply selection are immutable')
            selected = {**doc,**fields}
            if selected!=doc:save(self.record(ident),selected)
            return selected

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
        with self.lock:
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
            source = ('context:' if doc['reply'].get('context_only') else '') + doc['reply']['source']
            headers.update({'X-Reply-Source':source,'X-Reply-Revision':doc['reply']['revision']})
        # The caller invokes this only on an explicit Send/Retry gesture.
        result = transport.json('POST','/api/interaction/voice',clip,headers)
        if result.get('id') != ident:
            raise Unavailable('Saved recording admission remains unconfirmed')
        return self.remote(ident,result)

    def remote(self, ident, result):
        with self.lock:
            previous = self.read(ident)
            if previous.get('remote_terminal'):
                return {'id':ident,'phase':previous['remote_phase'],'retired':previous.get('remote_retired',False),
                    'routing':{'phase':previous['remote_route']} if previous.get('remote_route') else None}
            phase = result['phase']
            route = (result.get('routing') or {}).get('phase')
            terminal = phase in ('accepted','answered','awaiting_target','retained_unsent','uncertain','preparation_unconfirmed') or result.get('retired') or route in ('accepted','retained_unsent','uncertain','cancelled','rejected')
            self.update(ident,phase='recorded_remote',remote_phase=phase,remote_terminal=bool(terminal),
                remote_retired=bool(result.get('retired')),remote_route=route)
            return result

    def inspect(self, transport, ident):
        doc = self.read(ident)
        if doc['device_identity'] != transport.identity:
            raise Unavailable('Saved clip belongs to an earlier enrollment')
        result = transport.json('GET','/api/interaction/inputs/'+ident)
        if result.get('id') != ident:
            raise Unavailable('Recording receipt selection changed')
        return self.remote(ident,result)


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
                raise Unavailable('Selected local capture is unavailable',status=409 if value.get('ok') is False else None)
            return value
        except (OSError, ValueError, subprocess.TimeoutExpired):
            raise Unavailable('Local device state is unavailable') from None

    def status(self):
        return self.command('status')['snapshot']

    def start(self, output):
        return self.command('capture','audio',120,output)

    def stop(self, output):
        return self.command('stop-capture',output)


class Companion:
    """One foreground actor; only an explicit button may submit original speech."""
    def __init__(self, transport, drafts, capture, playback, emit=lambda _value:None):
        if not isinstance(playback,list) or not 1 <= len(playback) <= 12 or any(not isinstance(s,str) or not s or '\0' in s for s in playback):
            raise ValueError('Select an owner-configured audio player argv')
        self.transport, self.drafts, self.capture = transport, drafts, capture
        self.playback, self.emit = playback, emit
        self.path = drafts.root/'client.json'
        self.state = json.loads(private_bytes(self.path,512*1024)) if self.path.exists() else {
            'schema':'daimon-integrators.companion-state/v1','device_identity':transport.identity,
            'current':None,'pending':[],'cursor':None,'agenda':[],'seen':{}}
        if self.state.get('device_identity') != transport.identity:
            raise Unavailable('Saved conversation belongs to another enrollment; preserve it before selecting a new profile')
        self.state.setdefault('deferred',{})
        self.binding = self.state.get('binding')
        self.connected = self.can_speak = self.armed = self.closed = self.busy = False
        self.snapshot, self.focus = None, None
        self.status = 'Connecting to your conversation.'
        self.generation = 0
        self.play_offset = 0
        self.player = self.player_start = self.play_task = self.capture_start = self.source = None
        self.request_cursor = 0
        self.lock = asyncio.Lock()
        self.last_state = None

    def retain(self):
        current = json.dumps(self.state,sort_keys=True)
        if current!=self.last_state:
            save(self.path,self.state)
            self.last_state = current

    def publish(self):
        current = self.state['current']
        doc = self.drafts.read(current) if current else None
        phase = doc['phase'] if doc else 'playing' if self.source else 'ready' if self.connected else 'reconnecting'
        button = 'Send voice' if phase in ('capturing','capture_preparing','ready_to_send') else 'Retry saved voice' if phase=='submission_unconfirmed' else 'Talk'
        question = next((s for s in self.state['agenda'] if s['kind']=='native_question' and not s.get('paused')),None)
        if not self.armed and question and not current:
            button = 'Listen to question'
        local_ready = self.snapshot is not None
        status = self.status
        if not local_ready and not current and not self.source:
            status = 'Local microphone control is unavailable.'
        self.emit({'schema':'daimon-integrators.companion-view/v1','phase':phase,
            'status':status,'button':button,'enabled':not self.closed and not self.busy and (bool(current) or self.connected and self.can_speak and local_ready),
            'connected':self.connected,'source':self.source.get('label','') if self.source else '',
            'focus':self.focus.get('label','') if self.focus else '',
            'capture':self.snapshot.get('managed') if self.snapshot else None,
            'observation':self.snapshot.get('observation') if self.snapshot else None,
            'pending_messages':len(self.state['pending'])})

    async def api(self, method, path, body=None):
        return await asyncio.to_thread(self.transport.json,method,path,body)

    def enqueue(self, source):
        if (not isinstance(source.get('id'),str) or not re.fullmatch(r'(?:request:[a-f0-9]{32}|delivery:[a-f0-9-]{36}|native:[1-9][0-9]{0,18})',source['id'])
                or not re.fullmatch(r'[a-f0-9]{64}',source.get('revision',''))
                or source.get('current') is False or source.get('correction_pending')):
            return True
        if (self.state['seen'].get(source['id']) == source['revision']
                or self.state['deferred'].get(source['id'],{}).get('revision') == source['revision']):
            return True
        if any(s['id']==source['id'] and s['revision']==source['revision'] for s in self.state['agenda']):
            return True
        if len(self.state['agenda'])>=128 and not any(s['id']==source['id'] for s in self.state['agenda']):
            return False
        label = str(source.get('label','Conversation'))[:200]
        source = {k:source[k] for k in ('id','revision','kind')}
        # The label is presentation only, never an endpoint or command.
        source['label'] = label
        self.state['agenda'] = [s for s in self.state['agenda'] if s['id']!=source['id']]
        self.state['agenda'].append(source)
        return True

    def defer(self, source, reason):
        self.state['deferred'][source['id']] = {'revision':source['revision'],'reason':reason}
        self.state['deferred'] = dict(list(self.state['deferred'].items())[-256:])
        self.state['agenda'] = [s for s in self.state['agenda'] if s is not source]

    async def stop_audio(self):
        self.generation += 1
        if self.source:
            self.defer(self.source,'playback_paused')
            self.retain()
        self.source = None
        player = self.player
        if self.player_start:
            try:player = await asyncio.shield(self.player_start)
            except OSError:pass
        await self.stop_player(player)

    async def stop_player(self, player):
        if player and player.returncode is None:
            try:
                os.killpg(player.pid,signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                await asyncio.wait_for(player.wait(),2)
            except TimeoutError:
                try:os.killpg(player.pid,signal.SIGKILL)
                except ProcessLookupError:pass
                await player.wait()

    def remember_remote(self, ident, result):
        phase = result.get('phase')
        self.state['current'] = None if self.state['current']==ident else self.state['current']
        terminal = phase in ('accepted','answered','awaiting_target','retained_unsent','uncertain','preparation_unconfirmed') or result.get('retired') or (result.get('routing') or {}).get('phase') in ('accepted','retained_unsent','uncertain','cancelled','rejected')
        if not terminal and ident not in self.state['pending']:
            self.state['pending'].append(ident)
        if terminal and ident in self.state['pending']:
            self.state['pending'].remove(ident)
        if not self.state['current']:
            self.status = 'Answer recorded. Work may continue.' if phase=='answered' else 'Message saved; sessions may continue.'
        self.retain()

    async def stop_capture(self, ident):
        await asyncio.to_thread(self.capture.stop,self.drafts.audio(ident))
        deadline = time.monotonic()+6
        while True:
            snapshot = await asyncio.to_thread(self.capture.status)
            selected = snapshot['managed']
            if selected.get('output') != str(self.drafts.audio(ident)):
                raise Unavailable('This capture changed. Its saved recording was not sent.',status=409)
            if selected.get('pid') is None and selected.get('phase') not in ('preparing','capturing','stopping'):
                return snapshot
            if time.monotonic()>deadline:
                raise Unavailable('Microphone stop is unconfirmed. Nothing was sent.')
            await asyncio.sleep(.05)

    async def button(self, discard=False):
        if self.closed or self.busy:
            return
        async with self.lock:
            self.busy = True
            created = None
            try:
                audible = self.source if (self.source and self.source['kind'] in ('native_question','worker_result')
                    and self.player and self.player.returncode is None) else None
                await self.stop_audio()
                ident = self.state['current']
                if discard:
                    if ident:
                        doc = self.drafts.read(ident)
                        if doc['phase'] in ('capture_preparing','capturing'):
                            snapshot = await asyncio.to_thread(self.capture.status)
                            if snapshot['managed'].get('output')==str(self.drafts.audio(ident)):
                                await self.stop_capture(ident)
                            self.drafts.update(ident,phase='discarded')
                        # Keep any uncertain server effect inspectable; this is
                        # a local gesture, never a withdrawal of submitted work.
                        if doc['phase']=='submission_unconfirmed' and ident not in self.state['pending']:
                            self.state['pending'].append(ident)
                        self.state['current'] = None
                        self.status = 'Recording kept locally. You can start another message.'
                        self.retain()
                    return
                was_armed = self.armed
                self.armed = True
                if ident:
                    doc = self.drafts.read(ident)
                    if doc['phase'] in ('capture_preparing','capturing'):
                        snapshot = await self.stop_capture(ident)
                        selected = snapshot['managed']
                        if selected.get('phase')=='cancelled_before_capture':
                            self.drafts.update(ident,phase='discarded')
                            self.state['current']=None
                            self.retain()
                            return
                        self.drafts.finalize(ident,snapshot)
                    self.status = 'Saving your voice…'
                    self.publish()
                    result = await asyncio.to_thread(self.drafts.submit,self.transport,ident)
                    self.remember_remote(ident,result)
                    return
                if not self.connected or not self.can_speak or not self.binding:
                    raise Unavailable('Reconnect before starting a recording.')
                question = next((s for s in self.state['agenda'] if s['kind']=='native_question' and not s.get('paused')),None)
                if question and not self.focus and not was_armed:
                    self.launch_audio(question)
                    return
                if len(self.state['pending']) >= 64:
                    raise Unavailable('Saved message receipts need attention before recording again.')
                reference = ({'source':audible['id'],'revision':audible['revision']} if audible else
                    {k:self.focus[k] for k in ('source','revision')} if self.focus else None)
                if reference:reference['context_only'] = True
                self.focus = None
                doc = self.drafts.begin(self.transport.identity,self.binding,reference)
                created = doc['id']
                self.state['current'] = doc['id']
                self.retain()  # Identity and frozen reply precede acquisition.
                self.capture_start = asyncio.create_task(asyncio.to_thread(self.capture.start,self.drafts.audio(doc['id'])))
                await asyncio.shield(self.capture_start)
                self.status = 'Preparing microphone. Press again to finish.'
            except (Unavailable,OSError,ValueError,KeyError) as error:
                if created and isinstance(error,Unavailable) and error.status==409:
                    self.drafts.update(created,phase='capture_refused')
                    self.state['current'] = None
                    self.retain()
                self.status = str(error) if isinstance(error,Unavailable) else 'Device operation unconfirmed. Your saved input is retained.'
            finally:
                self.busy = False
                self.publish()

    async def observe_capture(self):
        self.snapshot = await asyncio.to_thread(self.capture.status)
        ident = self.state['current']
        if not ident or self.busy:
            return
        doc = self.drafts.read(ident)
        if doc['phase'] not in ('capture_preparing','capturing'):
            return
        selected = self.snapshot['managed']
        if selected.get('output') != str(self.drafts.audio(ident)):
            self.status = 'Selected microphone state is unavailable. Recording was not sent.'
            return
        if selected.get('phase')=='capturing':
            self.drafts.update(ident,phase='capturing')
            self.status = 'Listening. Press again to send voice.'
        elif selected.get('phase') in ('stopped','completed'):
            self.drafts.finalize(ident,self.snapshot)
            self.status = 'Recording finished. Press Send voice to send it.'
        elif selected.get('phase') in ('failed','cancelled_before_capture'):
            self.status = 'Capture did not finish. Hold the button to start another message.'

    async def poll(self, observe_local=True):
        if observe_local:
            try:
                await self.observe_capture()
            except (Unavailable,OSError,ValueError,KeyError):
                self.snapshot = None
                self.status = 'Local microphone state unavailable.'
        try:
            cursor = self.state['cursor']
            query = urllib.parse.urlencode({'delivery':cursor['delivery'] if cursor else 0,
                'native':cursor['native'] if cursor else 0,'direction':'after' if cursor else 'before'})
            page = await self.api('GET','/api/interaction?'+query)
            if self.binding and self.binding!=page['binding']:
                await self.stop_audio()
                self.focus = None
                self.state.update(cursor=None,agenda=[],seen={})
                cursor = None
            self.binding = page['binding']
            self.state['binding'] = self.binding
            self.connected = True
            self.can_speak = page.get('can_speak') is True
            if cursor and len(self.state['agenda']) < 96:
                admitted = True
                for source in sorted(page['items'],key=lambda s:(s.get('created_at',''),s['id'])):
                    admitted = self.enqueue(source) and admitted
                if admitted:self.state['cursor'] = page['cursor']
            elif cursor is None:
                self.state['cursor'] = page['newest']
            # Refresh recent revisions without letting metadata skip unread
            # forward pages. Questions have their own bounded rotating cursor.
            recent = await self.api('GET','/api/interaction?delivery=0&native=0&direction=before')
            if recent['binding'] != self.binding:
                raise Unavailable('Conversation changed while refreshing.')
            for source in recent['items']:
                if source['id'] in self.state['seen']:
                    self.enqueue(source)
            if self.source:
                selected, generation = self.source, self.generation
                query = urllib.parse.urlencode({'source':selected['id'],'revision':selected['revision'],'offset':self.play_offset})
                try:
                    await self.api('GET','/api/interaction/part?'+query)
                except Unavailable as error:
                    if self.source is selected and self.generation==generation:
                        await self.stop_audio()
                    if error.status!=409:raise
            requests = await self.api('GET','/api/interaction/requests?after='+str(self.request_cursor))
            for request in requests['items']:
                if request.get('kind')=='question' and request.get('actionable'):
                    if self.source and self.source['id']=='request:'+request['id'] and self.source['revision']!=request['revision']:
                        await self.stop_audio()
                    self.enqueue({'id':'request:'+request['id'],'revision':request['revision'],
                        'kind':'native_question','label':request.get('label','Conversation')})
            self.request_cursor = requests['cursor'] if requests['more'] else 0
            ids = ([self.state['current']] if self.state['current'] and self.drafts.read(self.state['current'])['phase']=='submission_unconfirmed' else []) + self.state['pending'][:8]
            for ident in dict.fromkeys(ids):
                try:
                    result = await asyncio.to_thread(self.drafts.inspect,self.transport,ident)
                    self.remember_remote(ident,result)
                except Unavailable:
                    pass
            checked = set(ids)
            self.state['pending'] = [i for i in self.state['pending'] if i not in checked] + [i for i in self.state['pending'] if i in checked]
            if not self.busy and not self.state['current'] and not self.source:
                self.status = 'Ready. Press Talk.' if not self.state['pending'] else 'Message saved; sessions may continue. Press Talk for another message.'
            if not self.can_speak:
                self.status = 'Voice input is not configured for this device.'
            self.retain()
            self.pump()
        except (Unavailable,OSError,ValueError,KeyError,TypeError):
            self.connected = False
            self.status = 'Connection unavailable. Recordings and work are retained.'
            await self.stop_audio()
        self.publish()

    def launch_audio(self, source):
        if self.play_task and not self.play_task.done():
            return
        # Preparing/playing another source cannot keep a previous question as
        # the context of a new utterance. Audible supported sources take over.
        self.focus = None
        self.source = source
        self.play_task = asyncio.create_task(self.play(source,self.generation))

    def pump(self):
        if not self.armed or self.closed or self.busy or self.state['current'] or not self.connected or self.play_task and not self.play_task.done():
            return
        eligible = [s for s in self.state['agenda'] if not s.get('paused') and not s.get('audio_unavailable')]
        if eligible:
            self.launch_audio(next((s for s in eligible if s['kind']=='native_question'),eligible[0]))

    async def play(self, source, generation):
        offset = 0
        def current():return not self.closed and self.generation==generation and self.source is source
        try:
            while current():
                self.play_offset = offset
                selection = {'source':source['id'],'revision':source['revision'],'offset':offset}
                self.status = 'Preparing audio from '+source.get('label','your conversation')+'.'
                self.publish()
                data, headers = await asyncio.to_thread(self.transport.request,'POST','/api/interaction/audio',selection,None,8*1024*1024,145)
                if not current():return
                if (headers.get('x-presentation-source')!=source['id'] or headers.get('x-presentation-revision')!=source['revision']
                        or headers.get('x-presentation-offset')!=str(offset)
                        or hashlib.sha256(data).hexdigest()!=headers.get('x-audio-sha256')):
                    raise Unavailable('Audio source could not be confirmed.')
                query = urllib.parse.urlencode(selection)
                part = await self.api('GET','/api/interaction/part?'+query)
                if not current():return
                if (part.get('id')!=source['id'] or part.get('revision')!=source['revision']
                        or headers.get('x-presentation-next-offset')!=('' if part['next_offset'] is None else str(part['next_offset']))):
                    raise Unavailable('Audio continuation changed.')
                audio = self.drafts.root/'playback.wav'
                fd,name = tempfile.mkstemp(prefix='.audio-',dir=self.drafts.root)
                with os.fdopen(fd,'wb') as stream:stream.write(data)
                os.replace(name,audio)
                self.player_start = asyncio.create_task(asyncio.create_subprocess_exec(*self.playback,str(audio),
                    stdin=asyncio.subprocess.DEVNULL,stdout=asyncio.subprocess.DEVNULL,
                    stderr=asyncio.subprocess.DEVNULL,start_new_session=True))
                player = await self.player_start
                self.player = player
                if not current():
                    await self.stop_player(player)
                    return
                self.status = 'Playing '+source.get('label','your conversation')+'. Press Talk to speak.'
                self.publish()
                try:code = await asyncio.wait_for(player.wait(),125)
                except TimeoutError:
                    await self.stop_player(player)
                    raise Unavailable('Audio playback did not finish.') from None
                if not current():return
                if code:raise Unavailable('Audio player is unavailable. You can still speak.')
                after = await self.api('GET','/api/interaction/part?'+query)
                if after!=part:raise Unavailable('This reply changed during playback.',status=409)
                if not current():return
                next_offset = part['next_offset']
                if next_offset is None:break
                if type(next_offset) is not int or next_offset<=offset:
                    raise Unavailable('Audio continuation is unavailable.')
                offset = next_offset
            if not current():return
            self.state['seen'][source['id']] = source['revision']
            self.state['seen'] = dict(list(self.state['seen'].items())[-256:])
            self.state['agenda'] = [s for s in self.state['agenda'] if s is not source]
            if source['kind']=='native_question':
                self.focus = {'source':source['id'],'revision':source['revision'],'label':source.get('label','Question')}
                self.status = 'Question finished. Press Talk to continue.'
            else:
                if source['kind']=='worker_result':
                    self.focus = {'source':source['id'],'revision':source['revision'],'label':source.get('label','Work session')}
                self.status = 'Reply finished. Press Talk to continue.'
            self.retain()
        except (Unavailable,OSError,ValueError,KeyError,TypeError) as error:
            if current():
                if isinstance(error,Unavailable) and error.status==409:
                    self.state['agenda'] = [s for s in self.state['agenda'] if s is not source]
                else:self.defer(source,'audio_unavailable')
                self.status = 'Audio unavailable. Speak to continue; the original reply is retained.'
                self.retain()
        finally:
            if self.source is source:self.source = None
            self.publish()

    async def close(self):
        if self.closed:return
        self.closed = True
        await self.stop_audio()
        if self.capture_start:
            try:await asyncio.shield(self.capture_start)
            except (Unavailable,OSError):pass
        ident = self.state['current']
        if ident:
            doc = self.drafts.read(ident)
            if doc['phase'] in ('capture_preparing','capturing'):
                try:
                    await self.stop_capture(ident)
                    self.drafts.update(ident,phase='discarded')
                    self.state['current']=None
                    self.retain()
                except (Unavailable,OSError):
                    self.status = 'Microphone stop unconfirmed; the panel retains its actual state.'
        self.publish()


async def run(config):
    transport = Transport(config['credential_file'])
    drafts = Drafts(config['state_dir'])
    def emit(value):print(json.dumps(value),flush=True)
    companion = Companion(transport,drafts,Capture(config.get('integrator_binary','/usr/local/bin/daimon-integrators')),
        config.get('playback_command',['pw-play']),emit)
    async def observe():
        while not companion.closed:
            await companion.poll(observe_local=False)
            await asyncio.sleep(2)
    async def local():
        while not companion.closed:
            try:
                await companion.observe_capture()
            except (Unavailable,OSError,ValueError,KeyError):
                companion.snapshot = None
            companion.publish()
            await asyncio.sleep(.25 if companion.state['current'] else 1)
    observer = asyncio.create_task(observe())
    local_observer = asyncio.create_task(local())
    actions = set()
    try:
        while True:
            line = await asyncio.to_thread(__import__('sys').stdin.readline)
            if not line:break
            try:action = json.loads(line).get('action')
            except (ValueError,AttributeError):continue
            if action=='close':break
            if action in ('button','discard'):
                task = asyncio.create_task(companion.button(action=='discard'))
                actions.add(task);task.add_done_callback(actions.discard)
    finally:
        await companion.close()
        observer.cancel()
        local_observer.cancel()
        for task in list(actions):task.cancel()
        await asyncio.gather(observer,local_observer,*actions,return_exceptions=True)


if __name__=='__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',required=True,type=Path)
    args = parser.parse_args()
    lease = None
    try:
        config = json.loads(private_bytes(args.config))
        if config.get('schema') != 'daimon-integrators.companion-config/v1':
            raise ValueError('Unsupported companion configuration')
        root = directory(config['state_dir'])
        lease = os.open(root/'foreground.lock',os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK,0o600)
        meta = os.fstat(lease)
        if not stat.S_ISREG(meta.st_mode) or meta.st_uid!=os.getuid() or meta.st_mode & 0o077:
            raise ValueError('Private companion lease required')
        fcntl.flock(lease,fcntl.LOCK_EX | fcntl.LOCK_NB)
        # Keep the profile lease through asyncio's executor shutdown as well.
        asyncio.run(run(config))
    except BlockingIOError:
        print(json.dumps({'schema':'daimon-integrators.companion-view/v1','phase':'unavailable',
            'status':'This companion is already open.','enabled':False}),flush=True)
    except (Unavailable,OSError,ValueError,KeyError):
        print(json.dumps({'schema':'daimon-integrators.companion-view/v1','phase':'unavailable',
            'status':'Companion configuration unavailable. Saved inputs are retained.','enabled':False}),flush=True)
    finally:
        if lease is not None:os.close(lease)
