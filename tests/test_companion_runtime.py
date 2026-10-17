"""Foreground button/queue races with real owned player processes; zero inference."""
import asyncio
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
import urllib.parse
import test_companion_client as fixtures

client = fixtures.client


class Device:
    def __init__(self):
        self.starts = self.stops = 0
        self.entered, self.release = threading.Event(), threading.Event()
        self.release.set()
        self.managed = {'activity':'','phase':'idle','pid':None,'output':None}

    def status(self):
        return {'at':int(time.time()),'managed':dict(self.managed),'observation':{}}

    def start(self, output):
        self.entered.set(); self.release.wait(timeout=3)
        Path(output).write_bytes(b'finalized synthetic recorder output')
        self.starts += 1
        self.managed = {'activity':'recording','phase':'capturing','pid':123,'output':str(output)}
        return {'ok':True,'stage':'preparing'}

    def stop(self, output):
        if str(output)!=self.managed['output']:
            raise client.Unavailable('Another capture is current',status=409)
        self.stops += 1
        self.managed.update(phase='stopped',pid=None)
        return {'ok':True,'stage':'stop_requested'}


class Controller:
    identity = 'e'*64
    def __init__(self):
        self.question = True
        self.changed = False
        self.posts = []
        self.audio_calls = 0
        self.entered, self.release = threading.Event(), threading.Event()
        self.release.set()
        self.voice_phase = 'answered'
        self.originals = []
        self.source = {'id':'request:'+'a'*32,'revision':'b'*64,'kind':'native_question','label':'#Alpha'}

    def json(self, method, path, body=None, headers=None):
        url = urllib.parse.urlsplit(path)
        if url.path=='/api/interaction/voice':
            ident = headers['X-Input-Id']
            self.posts.append({'id':ident,'body':body,'headers':headers})
            self.question = False
            return {'id':ident,'phase':self.voice_phase,'routing':None}
        if url.path=='/api/interaction':
            return {'binding':'c'*64,'can_speak':True,'can_listen':True,
                'items':[],'cursor':{'native':0,'delivery':0},'newest':{'native':0,'delivery':0}}
        if url.path=='/api/interaction/requests':
            return {'items':[{'id':'a'*32,'revision':'b'*64,'kind':'question','actionable':True,'label':'#Alpha'}] if self.question else [],'cursor':1,'more':False}
        if url.path=='/api/interaction/part':
            if self.changed:raise client.Unavailable('Question changed',status=409)
            offset = int(urllib.parse.parse_qs(url.query)['offset'][0])
            return {**self.source,'offset':offset,'text':'Selected current question','next_offset':10 if offset==0 else None}
        if url.path=='/api/interaction/inputs':
            return {'items':self.originals,'cursor':0,'more':False}
        if url.path.startswith('/api/interaction/inputs/'):
            return {'id':url.path.rsplit('/',1)[-1],'phase':'preparing','routing':None}
        raise AssertionError(path)

    def request(self, method, path, body=None, headers=None, bound=None, timeout=None):
        assert method=='POST' and path=='/api/interaction/audio'
        self.audio_calls += 1
        self.entered.set();self.release.wait(timeout=3)
        data=b'synthetic source-linked player data'
        return data,{'x-presentation-source':body['source'],'x-presentation-revision':body['revision'],
            'x-presentation-offset':str(body['offset']),'x-presentation-next-offset':'10' if body['offset']==0 else '',
            'x-audio-sha256':hashlib.sha256(data).hexdigest()}


class Runtime(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.controller, self.device = Controller(), Device()
        self.drafts = client.Drafts(Path(self.temporary.name)/'state')
        self.views = []
        self.runtime = client.Companion(self.controller,self.drafts,self.device,
            [sys.executable,'-c','import time;time.sleep(.03)'],self.views.append)
        await self.runtime.poll()

    async def asyncTearDown(self):
        self.controller.release.set();self.device.release.set()
        await self.runtime.close()
        if self.runtime.play_task:await self.runtime.play_task
        self.temporary.cleanup()

    async def test_no_startup_sensor_or_audio_and_multipart_question_then_contextual_voice_original(self):
        self.assertEqual(self.device.starts,0)
        self.assertEqual(self.controller.audio_calls,0)
        self.assertEqual(self.views[-1]['button'],'Listen to question')
        await self.runtime.button()
        await self.runtime.play_task
        self.assertEqual(self.controller.audio_calls,2)
        self.assertEqual(self.runtime.focus['source'],self.controller.source['id'])
        self.assertEqual(self.device.starts,0)
        await self.runtime.button()
        ident = self.runtime.state['current']
        frozen = self.drafts.read(ident)['reply']
        self.assertEqual(frozen,{'source':self.controller.source['id'],'revision':self.controller.source['revision'],'context_only':True})
        self.assertEqual(self.controller.posts,[])
        await self.runtime.button()
        self.assertEqual(len(self.controller.posts),1)
        self.assertEqual(self.controller.posts[0]['headers']['X-Reply-Source'],'context:'+frozen['source'])
        self.assertEqual(self.drafts.read(ident)['remote_phase'],'answered')
        self.assertIsNone(self.runtime.state['current'])

    async def test_destination_question_is_discovered_played_and_retained_as_context(self):
        self.controller.question=False
        self.controller.source={'id':'input:11111111-1111-4111-8111-111111111111',
            'revision':'a'*64,'kind':'route_clarification','label':'Destination needed'}
        self.controller.originals=[{'id':self.controller.source['id'][6:],'phase':'awaiting_target',
            'retired':False,'clarification':dict(self.controller.source)}]
        self.runtime.state['agenda']=[]
        await self.runtime.poll()
        self.assertEqual(self.views[-1]['button'],'Listen to question')
        await self.runtime.button();await self.runtime.play_task
        self.assertEqual(self.device.starts,0)
        await self.runtime.button()
        self.assertEqual(self.drafts.read(self.runtime.state['current'])['reply'],
            {'source':self.controller.source['id'],'revision':'a'*64,'context_only':True})
        self.controller.voice_phase='accepted'
        await self.runtime.button()
        self.assertEqual(self.controller.posts[0]['headers']['X-Reply-Source'],'context:'+self.controller.source['id'])

    async def test_other_original_cannot_install_a_destination_descriptor_under_this_receipt(self):
        self.runtime.state['agenda']=[]
        self.runtime.enqueue_clarification({'id':'22222222-2222-4222-8222-222222222222',
            'phase':'awaiting_target','clarification':{'id':'input:11111111-1111-4111-8111-111111111111',
            'revision':'a'*64,'kind':'route_clarification','label':'Other original'}})
        self.assertEqual(self.runtime.state['agenda'],[])

    async def test_barge_in_during_held_synthesis_never_plays_or_selects_old_source(self):
        self.controller.question=False;self.controller.release.clear()
        self.runtime.state['agenda']=[]
        self.runtime.launch_audio(dict(self.controller.source))
        self.assertTrue(await asyncio.to_thread(self.controller.entered.wait,2))
        await self.runtime.button()
        self.assertEqual(self.device.starts,1)
        self.controller.release.set();await self.runtime.play_task
        self.assertIsNone(self.runtime.player)
        self.assertIsNone(self.runtime.focus)
        self.assertEqual(self.runtime.state['seen'],{})

    async def test_other_pending_questions_cannot_steal_an_armed_recording_gesture(self):
        self.runtime.armed=True
        self.runtime.enqueue({'id':'request:'+'f'*32,'revision':'a'*64,'kind':'native_question','label':'#Beta'})
        await self.runtime.button()
        self.assertEqual(self.device.starts,1)
        self.assertEqual(self.controller.audio_calls,0)
        self.assertIsNotNone(self.runtime.state['current'])

    async def test_barge_in_on_a_new_question_never_answers_the_previous_one(self):
        await self.runtime.button()
        await self.runtime.play_task
        self.assertIsNotNone(self.runtime.focus)
        self.controller.release.clear()
        self.controller.entered.clear()
        self.runtime.launch_audio({**self.controller.source,'id':'request:'+'f'*32})
        self.assertTrue(await asyncio.to_thread(self.controller.entered.wait,2))
        await self.runtime.button()
        self.assertIsNone(self.drafts.read(self.runtime.state['current'])['reply'])
        self.controller.release.set()
        await self.runtime.play_task

    async def test_barge_in_during_audible_question_keeps_context_without_answer_selection(self):
        self.runtime.playback=[sys.executable,'-c','import time;time.sleep(1)']
        await self.runtime.button()
        for _ in range(100):
            if self.runtime.player:break
            await asyncio.sleep(.01)
        self.assertIsNotNone(self.runtime.player)
        await self.runtime.button()
        original=self.drafts.read(self.runtime.state['current'])
        self.assertEqual(original['reply'],{'source':self.controller.source['id'],
            'revision':self.controller.source['revision'],'context_only':True})
        self.assertIsNotNone(self.runtime.player.returncode)
        self.assertEqual(self.device.starts,1)

    async def test_audible_worker_takes_precedence_over_previous_question_or_worker(self):
        await self.runtime.button();await self.runtime.play_task
        old_question=dict(self.runtime.focus)
        self.runtime.playback=[sys.executable,'-c','import time;time.sleep(1)']
        for old in (old_question,{'source':'delivery:11111111-1111-1111-1111-111111111111',
                'revision':'a'*64,'label':'#Alpha'}):
            self.runtime.focus=old
            self.controller.source={'id':'delivery:22222222-2222-2222-2222-222222222222',
                'revision':'b'*64,'kind':'worker_result','label':'#Beta'}
            self.runtime.player=None
            self.runtime.launch_audio(dict(self.controller.source))
            for _ in range(100):
                if self.runtime.player:break
                await asyncio.sleep(.01)
            self.assertIsNotNone(self.runtime.player)
            await self.runtime.button()
            hint=self.drafts.read(self.runtime.state['current'])['reply']
            self.assertEqual(hint,{'source':self.controller.source['id'],'revision':'b'*64,'context_only':True})
            await self.runtime.play_task;await self.runtime.button(discard=True)

    async def test_integrating_reply_barge_in_keeps_an_original_without_an_unsupported_hint(self):
        self.runtime.armed=True
        self.runtime.focus={'source':'request:'+'a'*32,'revision':'b'*64,'label':'Old question'}
        self.controller.source={'id':'native:3','revision':'c'*64,'kind':'integrator_answer','label':'Integrator'}
        self.runtime.playback=[sys.executable,'-c','import time;time.sleep(1)']
        self.runtime.launch_audio(dict(self.controller.source))
        for _ in range(100):
            if self.runtime.player:break
            await asyncio.sleep(.01)
        self.assertIsNotNone(self.runtime.player)
        await self.runtime.button()
        self.assertIsNone(self.drafts.read(self.runtime.state['current'])['reply'])
        self.assertEqual(self.device.starts,1)
        await self.runtime.play_task

    async def test_question_changed_during_real_player_lifetime_cannot_install_reply_focus(self):
        self.runtime.playback=[sys.executable,'-c','import time;time.sleep(.15)']
        await self.runtime.button()
        for _ in range(100):
            if self.runtime.player:break
            await asyncio.sleep(.01)
        self.assertIsNotNone(self.runtime.player)
        self.controller.changed=True
        await self.runtime.play_task
        self.assertIsNone(self.runtime.focus)
        self.assertEqual(self.runtime.state['seen'],{})

    async def test_close_waits_for_late_capture_start_then_stops_only_that_capture(self):
        self.controller.question=False;self.runtime.state['agenda']=[]
        self.device.release.clear()
        button=asyncio.create_task(self.runtime.button())
        self.assertTrue(await asyncio.to_thread(self.device.entered.wait,2))
        closing=asyncio.create_task(self.runtime.close())
        await asyncio.sleep(.02)
        self.assertFalse(closing.done())
        self.device.release.set()
        await button;await closing
        self.assertEqual((self.device.starts,self.device.stops),(1,1))
        self.assertIsNone(self.device.managed['pid'])
        self.assertEqual(self.controller.posts,[])

    async def test_player_start_race_is_stopped_before_new_microphone_capture(self):
        self.controller.question=False;self.runtime.state['agenda']=[]
        entered,release=asyncio.Event(),asyncio.Event()
        original=asyncio.create_subprocess_exec
        async def held(*args,**kwargs):
            entered.set();await release.wait()
            return await original(*args,**kwargs)
        with patch('asyncio.create_subprocess_exec',held):
            self.runtime.launch_audio(dict(self.controller.source))
            await entered.wait()
            button=asyncio.create_task(self.runtime.button())
            await asyncio.sleep(.02)
            self.assertEqual(self.device.starts,0)
            release.set();await button;await self.runtime.play_task
        self.assertEqual(self.device.starts,1)
        self.assertIsNotNone(self.runtime.player.returncode)
        self.assertIsNone(self.runtime.focus)

    async def test_new_voice_can_be_recorded_while_earlier_asr_is_preparing(self):
        self.controller.question=False;self.controller.voice_phase='preparing';self.runtime.state['agenda']=[]
        await self.runtime.button();first=self.runtime.state['current'];await self.runtime.button()
        self.assertIn(first,self.runtime.state['pending'])
        await self.runtime.button();second=self.runtime.state['current']
        self.assertNotEqual(first,second)
        self.assertEqual(self.device.starts,2)
        self.assertEqual(len(self.controller.posts),1)


if __name__=='__main__':
    unittest.main()
