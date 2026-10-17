"""Actual GTK, TLS, controller and native question answer; synthetic media, zero models.

Requires a Telecodex source checkout/binary and its dashboard Python dependencies.
Only fixture-owned processes, credentials and state are used. No physical sensors.
"""
import argparse,ctypes as C,ctypes.util,json,os,pathlib,signal,socket,subprocess,sys,tempfile,threading,time
OUT=None
BINARY=None
TELECODEX=None
TELECODEX_BINARY=None
os.umask(0o077)
def wait(check,seconds=12):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  value=check()
  if value:return value
  time.sleep(.03)
 raise AssertionError('Owned fixture observation timed out')
def main():
 with tempfile.TemporaryDirectory(prefix='integrator-companion-ui-')as td:
  root=pathlib.Path(td);runtime=root/'run';runtime.mkdir();panel=runtime/'daimon-integrators';panel.mkdir();state=root/'state'
  cert,key=root/'cert.pem',root/'key.pem'
  subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(key),'-out',str(cert),'-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  lock=threading.RLock();record={'phase':'idle','activity':'','output':None,'pid':None};encoder=None;counts={'starts':0,'stops':0,'posts':[],'requests':0};done=threading.Event()
  listener=socket.socket(socket.AF_UNIX);listener.bind(str(panel/'control.sock'));listener.listen();listener.settimeout(.2)
  def serve_panel():
   nonlocal encoder
   while not done.is_set():
    try:conn,_=listener.accept()
    except socket.timeout:continue
    except OSError:break
    with conn:
     raw=bytearray()
     while block:=conn.recv(65536):raw.extend(block)
     message=json.loads(raw);action=message['action']
     with lock:
      if action=='capture':
       output=pathlib.Path(message['output']);assert output.parent==state/'captures'and message['mode']=='audio'
       encoder=subprocess.Popen(['ffmpeg','-hide_banner','-loglevel','error','-n','-re','-f','lavfi','-i','sine=frequency=420','-t','120','-c:a','aac',str(output)],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
       counts['starts']+=1;record.update(activity='recording',phase='capturing',output=str(output),pid=encoder.pid);reply={'ok':True,'stage':'preparing'}
      elif action=='stop_capture':
       if message['output']!=record['output']:reply={'ok':False}
       else:
        if encoder and encoder.poll()is None:os.killpg(encoder.pid,signal.SIGINT);encoder.wait(timeout=5);counts['stops']+=1
        record.update(phase='stopped',pid=None);reply={'ok':True,'stage':'stop_requested'}
      else:
       assert action=='status'
       reply={'ok':True,'snapshot':{'at':int(time.time()),'managed':dict(record),'observation':{'at':int(time.time()),'complete':True,'microphone_present':True,'microphone_running':record['pid']is not None}}}
      conn.sendall(json.dumps(reply).encode())
  thread=threading.Thread(target=serve_panel,daemon=True);thread.start()
  sys.path[:0]=[str(TELECODEX/'tests'),str(TELECODEX)]
  import uvicorn
  from conversation_cold_entry_http_io import ColdEntry,Browser
  from dashboard.app import create_app
  from dashboard.companion import Grants
  from dashboard.provider import Provider
  from dashboard.private_state import read_json
  from speech_render import Render
  journey=ColdEntry(str(TELECODEX_BINARY));speech=Render();browser=None;httpthread=None;server=None
  try:
   journey.prepare();speech.setUp()
   bound=socket.socket();bound.bind(('127.0.0.1',0));bound.listen(32)
   origin='https://localhost:'+str(bound.getsockname()[1])
   receiving={'receiver_config':str(journey.config),'state_dir':str(journey.root/'device-dashboard'),
    'origin':origin,'bot_id':123,'companion':{'enabled':True,'grants_dir':str(journey.root/'device-grants')},
    'speech':{'binding':str(speech.binding),'timeout_seconds':5}}
   provider=Provider(receiving);device=root/'device.json'
   Grants(receiving,provider).enroll('Synthetic native GTK fixture',device,3600)
   app=create_app(receiving,provider)
   @app.middleware('http')
   async def observe_posts(request,call_next):
    response=await call_next(request)
    if request.method=='POST'and request.url.path.endswith('/interaction/voice'):
     counts['posts'].append({'id':request.headers['x-input-id'],'status':response.status_code})
    return response
   # Match the receiving reverse proxy: strip /sessions before the ASGI app.
   # Starlette Mount retains it in request.url.path, unlike that deployment.
   async def prefix_app(scope,receive,send):
    assert scope['type']=='http'and scope['path'].startswith('/sessions/')
    scope={**scope,'path':scope['path'][len('/sessions'):],
     'raw_path':scope['raw_path'][len(b'/sessions'):],'root_path':''}
    await app(scope,receive,send)
   server=uvicorn.Server(uvicorn.Config(prefix_app,log_level='error',ssl_keyfile=str(key),ssl_certfile=str(cert),lifespan='off'))
   httpthread=threading.Thread(target=server.run,kwargs={'sockets':[bound]},daemon=True);httpthread.start();wait(lambda:server.started)
   from fastapi.testclient import TestClient
   enrollment=read_json(device)
   browser=Browser.__new__(Browser);browser.client=TestClient(app,base_url=origin)
   browser.headers={'origin':origin,'authorization':enrollment['authorization'],'x-dashboard-csrf':enrollment['csrf']}
   browser.binding=browser.get('/api/interaction')['binding']
   original,_=browser.text('#Alpha Work on this task while the conversation stays available.')
   journey.wait(lambda:journey.instructions(7)and journey.instructions(7)[0][1]=='accepted')
   _,_=browser.text('#Beta Work independently while Alpha asks its question.')
   journey.wait(lambda:journey.instructions(8)and journey.instructions(8)[0][1]=='accepted')
   worker=next(send for selected,send in journey.clients.values()if selected=='native-topic-7')
   worker({'id':'canonical-gtk-question','method':'item/tool/requestUserInput','params':{
    'threadId':'native-topic-7','turnId':journey.workers['native-topic-7'],'itemId':'gtk-question','isBlocking':True,
    'questions':[{'id':'scope','header':'Scope','question':'Which requirement should this work follow?','options':None}]}})
   journey.wait(lambda:journey.row("SELECT count(*) FROM human_interactions WHERE state='pending'")==[(1,)])
  except BaseException:
   done.set();listener.close();thread.join(timeout=2)
   if server:server.should_exit=True
   if httpthread:httpthread.join(timeout=5)
   if browser:browser.client.close()
   speech.tearDown()if hasattr(speech,'temporary')else None
   journey.close()
   raise
  config=root/'config.json';config.write_text(json.dumps({'schema':'daimon-integrators.companion-config/v1','credential_file':str(device),'state_dir':str(state),'integrator_binary':str(BINARY),'playback_command':['true']}));config.chmod(0o600)
  xvfb=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','800x600x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True);display=':'+xvfb.stdout.readline().strip()
  env=dict(os.environ,DISPLAY=display,XDG_RUNTIME_DIR=str(runtime),SSL_CERT_FILE=str(cert))
  log=(OUT/'gui.log').open('w');gui=subprocess.Popen([str(BINARY),'companion',str(config)],env=env,stdout=log,stderr=log)
  x=C.CDLL(ctypes.util.find_library('X11'));xt=C.CDLL(ctypes.util.find_library('Xtst'))
  x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d
  x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;desktop=x.XDefaultRootWindow(d)
  x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
  x.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)];x.XFree.argtypes=[C.c_void_p];x.XFlush.argtypes=[C.c_void_p]
  xt.XTestFakeMotionEvent.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_ulong];xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
  def window():
   r,p,k,n=C.c_ulong(),C.c_ulong(),C.POINTER(C.c_ulong)(),C.c_uint();x.XQueryTree(d,desktop,C.byref(r),C.byref(p),C.byref(k),C.byref(n));found=0
   for i in range(n.value):
    name=C.c_char_p()
    if x.XFetchName(d,k[i],C.byref(name))and name.value:
     if name.value==b'Daimon conversation':found=k[i]
     x.XFree(name)
   if k:x.XFree(k)
   return found
  def client_state():
   try:return json.loads((state/'client.json').read_text())
   except (OSError,ValueError):return {}
  def click(hold=.07):
   xt.XTestFakeMotionEvent(d,-1,310,175,0);xt.XTestFakeButtonEvent(d,1,1,0);x.XFlush(d);time.sleep(hold);xt.XTestFakeButtonEvent(d,1,0,0);x.XFlush(d)
  try:
   wid=wait(window);wait(lambda:client_state().get('binding'));time.sleep(.3)
   assert counts['starts']==0 and not counts['posts']and not speech.calls,'Startup opened media, synthesized or sent input'
   click();wait(lambda:bool(client_state().get('seen')))
   assert len(speech.calls)==1 and b'#Alpha.'in speech.calls[0][2]
   time.sleep(.3);click();wait(lambda:counts['starts']==1);time.sleep(.8)
   calls=len(speech.calls);journey.answer_worker(8,'Independent Beta result while the human is speaking.')
   journey.wait(lambda:journey.row('SELECT count(*) FROM interaction_deliveries')!=[(0,)])
   time.sleep(.3);assert len(speech.calls)==calls,'Another result played over an active microphone'

   subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-f','x11grab','-video_size','800x600','-i',display,'-frames:v','1',str(OUT/'listening.png')],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   click();wait(lambda:len(counts['posts'])==1);wait(lambda:not client_state().get('current'))
   journey.wait(lambda:any(v['id']=='canonical-gtk-question'for v in journey.native_answers))
   answer=next(v for v in journey.native_answers if v['id']=='canonical-gtk-question')['result']['answers']['scope']['answers'][0]
   assert '[Committed voice input]'in answer and 'Continue requirement17'in answer
   submitted=counts['posts'][0];assert submitted['status']==200 and browser.status(submitted['id'])['phase']=='answered'
   wait(lambda:len(speech.calls)>=2);wait(lambda:len(client_state().get('seen',{}))>=2)
   assert not journey.packets and journey.row('SELECT codex_thread_id FROM sessions WHERE thread_id=1')==[(None,)]
   time.sleep(.3)  # GTK tick observes the settled receipt before the next gesture.

   click();wait(lambda:counts['starts']==2);time.sleep(.4);click(1.2);wait(lambda:counts['stops']==2);time.sleep(.3)
   assert len(counts['posts'])==1 and counts['starts']==2,'Long hold submitted or restarted a recording'
   click();wait(lambda:counts['starts']==3);time.sleep(.4)
   class Data(C.Union):_fields_=[('b',C.c_char*20),('s',C.c_short*10),('l',C.c_long*5)]
   class Message(C.Structure):_fields_=[('type',C.c_int),('serial',C.c_ulong),('send_event',C.c_int),('display',C.c_void_p),('window',C.c_ulong),('message_type',C.c_ulong),('format',C.c_int),('data',Data)]
   class Event(C.Union):_fields_=[('message',Message),('pad',C.c_long*24)]
   x.XInternAtom.argtypes=[C.c_void_p,C.c_char_p,C.c_int];x.XInternAtom.restype=C.c_ulong;x.XSendEvent.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_long,C.POINTER(Event)]
   event=Event();event.message.type=33;event.message.display=d;event.message.window=wid;event.message.message_type=x.XInternAtom(d,b'WM_PROTOCOLS',0);event.message.format=32;event.message.data.l[0]=x.XInternAtom(d,b'WM_DELETE_WINDOW',0)
   x.XSendEvent(d,wid,0,0,C.byref(event));x.XFlush(d);gui.wait(timeout=24)
   assert gui.returncode==0 and counts['stops']==3 and len(counts['posts'])==1 and record['pid']is None
   proof={'result':'PASS','actualGTK':True,'realXTestButtonEvents':True,'realTLS':True,'actualFFmpegSyntheticCapture':True,'starts':counts['starts'],'selectedStops':counts['stops'],'voicePosts':len(counts['posts']),'noStartupMedia':True,'holdDiscardsWithoutPost':True,'closeStopsSelectedCapture':True,'physicalSensors':False,'models':0,'controller':'actual Telecodex native-controller/SQLite/HTTP; synthetic App Server and ASR peers','nativeQuestionAnswered':True,'parallelWorkerResultPlayed':True,'coldFrontUnchanged':True,'speechCalls':len(speech.calls)}
   (OUT/'proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof),flush=True)
  finally:
   (OUT/'client-final.json').write_text(json.dumps(client_state(),indent=2)+'\n')
   if gui.poll()is None:
    gui.terminate()
    try:gui.wait(timeout=3)
    except subprocess.TimeoutExpired:gui.kill();gui.wait()
   if encoder and encoder.poll()is None:os.killpg(encoder.pid,signal.SIGINT);encoder.wait(timeout=5)
   done.set();listener.close();thread.join(timeout=2);server.should_exit=True;httpthread.join(timeout=5);browser.client.close();speech.tearDown();journey.close();xvfb.terminate();xvfb.wait(timeout=3);log.close()
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--telecodex-source',type=pathlib.Path,required=True)
 parser.add_argument('--telecodex-binary',type=pathlib.Path,required=True)
 parser.add_argument('--binary',type=pathlib.Path,required=True)
 parser.add_argument('--output',type=pathlib.Path,required=True)
 args=parser.parse_args()
 TELECODEX=args.telecodex_source.resolve();TELECODEX_BINARY=args.telecodex_binary.resolve();BINARY=args.binary.resolve()
 OUT=args.output.resolve();OUT.mkdir(mode=0o700,parents=True,exist_ok=True)
 main()

