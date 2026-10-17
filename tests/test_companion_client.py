"""Real TLS/filesystem receipt recovery; fictional peers, no sensors or models."""
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading
import unittest
import urllib.request

SOURCE = Path(__file__).resolve().parents[1]/'tools/companion_client.py'
spec = importlib.util.spec_from_file_location('companion_client', SOURCE)
client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(client)


class Companion(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.certificates = tempfile.TemporaryDirectory()
        root = Path(cls.certificates.name)
        cls.certificate, cls.key = root/'certificate.pem', root/'key.pem'
        subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes',
            '-keyout',str(cls.key),'-out',str(cls.certificate),'-days','1',
            '-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost'],
            check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)

    @classmethod
    def tearDownClass(cls):
        cls.certificates.cleanup()

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.requests, self.receipts = [], {}
        self.lose = False
        self.redirect = False
        owner = self

        class Peer(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                data = self.rfile.read(int(self.headers['Content-Length']))
                ident = self.headers['X-Input-Id']
                owner.requests.append((self.command,self.path,dict(self.headers),data))
                owner.receipts[ident] = {'id':ident,'phase':'preparing'}
                if owner.lose:
                    self.connection.shutdown(socket.SHUT_RDWR)
                    self.connection.close()
                    return
                self.reply(owner.receipts[ident])

            def do_GET(self):
                owner.requests.append((self.command,self.path,dict(self.headers),None))
                if owner.redirect:
                    self.send_response(302)
                    self.send_header('Location',owner.origin+'/another-host-route')
                    self.end_headers()
                    return
                ident = self.path.rsplit('/',1)[-1]
                self.reply(owner.receipts.get(ident,{'binding':'b'*64}))

            def reply(self, value):
                raw=json.dumps(value).encode()
                self.send_response(200);self.send_header('Content-Length',str(len(raw)))
                self.end_headers();self.wfile.write(raw)

            def log_message(self,*_args):pass

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1',0),Peer)
        context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(self.certificate,self.key)
        self.server.socket=context.wrap_socket(self.server.socket,server_side=True)
        self.thread=threading.Thread(target=self.server.serve_forever,daemon=True)
        self.thread.start()
        self.origin='https://localhost:'+str(self.server.server_port)
        self.credential=self.root/'device.json'
        self.device={'schema':'telecodex.companion-device/v1','origin':self.origin,
            'prefix':'/sessions','authorization':'Companion '+'A'*43,'csrf':'B'*43}
        client.save(self.credential,self.device)
        self.transport=self.transport_for()
        self.drafts=client.Drafts(self.root/'drafts')

    def transport_for(self):
        transport=client.Transport(self.credential)
        context=ssl.create_default_context(cafile=str(self.certificate))
        transport.opener=urllib.request.build_opener(client.NoRedirect,
            urllib.request.HTTPSHandler(context=context))
        return transport

    def tearDown(self):
        self.server.shutdown();self.server.server_close();self.thread.join(timeout=2)
        self.temporary.cleanup()

    def finalized(self):
        reply={'source':'request:'+'c'*32,'revision':'d'*64}
        doc=self.drafts.begin(self.transport.identity,'b'*64,reply)
        media=self.drafts.audio(doc['id']);media.write_bytes(b'fixture finalized media bytes')
        media.chmod(0o644)  # Recorder output is contained in the private directory.
        snapshot={'managed':{'activity':'recording','phase':'stopped','pid':None,'output':str(media)}}
        self.drafts.finalize(doc['id'],snapshot)
        return doc,snapshot

    def test_lost_upload_receipt_recovers_after_restart_without_resending(self):
        doc,_=self.finalized();self.lose=True
        with self.assertRaises(client.Unavailable):self.drafts.submit(self.transport,doc['id'])
        self.assertEqual(self.drafts.read(doc['id'])['phase'],'submission_unconfirmed')
        self.assertEqual(len(self.requests),1)
        method,path,headers,data=self.requests[0]
        self.assertEqual((method,path),('POST','/sessions/api/interaction/voice'))
        self.assertEqual(headers['Authorization'],self.device['authorization'])
        self.assertEqual(headers['Origin'],self.origin)
        self.assertEqual(headers['X-Input-Id'],doc['id'])
        self.assertEqual(headers['X-Audio-Sha256'],hashlib.sha256(data).hexdigest())
        self.assertEqual(headers['X-Reply-Source'],doc['reply']['source'])
        self.drafts=client.Drafts(self.root/'drafts')
        result=self.drafts.inspect(self.transport_for(),doc['id'])
        self.assertEqual(result,{'id':doc['id'],'phase':'preparing'})
        self.assertEqual(len([r for r in self.requests if r[0]=='POST']),1)
        with self.assertRaises(ValueError):self.drafts.submit(self.transport,doc['id'])

    def test_late_preparing_observation_cannot_reopen_a_recorded_answer(self):
        doc,_=self.finalized()
        self.drafts.remote(doc['id'],{'id':doc['id'],'phase':'answered','routing':None})
        returned=self.drafts.remote(doc['id'],{'id':doc['id'],'phase':'preparing','routing':None})
        self.assertEqual(returned['phase'],'answered')
        self.assertTrue(self.drafts.read(doc['id'])['remote_terminal'])
        self.assertEqual(self.drafts.read(doc['id'])['remote_phase'],'answered')

    def test_terminal_destination_receipt_recovers_its_descriptor_after_restart(self):
        doc,_=self.finalized()
        descriptor={'id':'input:'+doc['id'],'revision':'c'*64,'kind':'route_clarification','label':'Destination needed'}
        receipt={'id':doc['id'],'phase':'awaiting_target','routing':None,'retired':False,'clarification':descriptor}
        self.drafts.remote(doc['id'],receipt)
        self.receipts[doc['id']]=receipt
        self.drafts=client.Drafts(self.root/'drafts')
        recovered=self.drafts.inspect(self.transport,doc['id'])
        self.assertEqual(recovered['clarification'],descriptor)
        self.assertEqual(self.drafts.read(doc['id'])['remote_phase'],'awaiting_target')
        self.assertFalse(any(r[0]=='POST'for r in self.requests))

    def test_explicit_identical_retry_preserves_original_clip_identity(self):
        doc,_=self.finalized();self.lose=True
        with self.assertRaises(client.Unavailable):self.drafts.submit(self.transport,doc['id'])
        self.lose=False;self.drafts.submit(self.transport,doc['id'])
        self.assertEqual(self.requests[0][3],self.requests[1][3])
        self.assertEqual(self.requests[0][2]['X-Input-Id'],self.requests[1][2]['X-Input-Id'])

    def test_credential_rotation_and_changed_clip_cannot_reassign_a_saved_original(self):
        doc,_=self.finalized()
        client.save(self.credential,{**self.device,'authorization':'Companion '+'C'*43})
        with self.assertRaises(client.Unavailable):self.drafts.submit(self.transport_for(),doc['id'])
        with self.assertRaises(client.Unavailable):self.transport.json('GET','/api/interaction')
        client.save(self.credential,self.device)
        self.drafts.audio(doc['id']).write_bytes(b'different immutable recording')
        with self.assertRaises(client.Unavailable):self.drafts.submit(self.transport,doc['id'])
        self.assertEqual(self.requests,[])

    def test_active_or_different_capture_cannot_be_committed(self):
        doc,snapshot=self.finalized()
        for change in ({'pid':123},{'phase':'capturing'},{'output':'/other-capture.m4a'}):
            with self.assertRaises(client.Unavailable):
                self.drafts.finalize(doc['id'],{'managed':{**snapshot['managed'],**change}})
        self.assertEqual(self.drafts.audio(doc['id']).stat().st_mode & 0o777,0o600)
        with self.assertRaises(ValueError):self.drafts.update(doc['id'],reply=None)
        self.assertEqual(self.requests,[])

    def test_redirects_and_caller_authentication_overrides_never_forward_credential(self):
        self.redirect=True
        with self.assertRaises(client.Unavailable):self.transport.json('GET','/api/interaction')
        self.assertEqual(len(self.requests),1)
        for path in ('https://other.test/api/interaction','/api/interaction/../state'):
            with self.assertRaises(ValueError):self.transport.json('GET',path)
        with self.assertRaises(ValueError):
            self.transport.request('GET','/api/interaction',headers={'Authorization':'other'})
        self.assertEqual(len(self.requests),1)


if __name__=='__main__':
    unittest.main()
