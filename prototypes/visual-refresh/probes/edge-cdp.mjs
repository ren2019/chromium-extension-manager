// Throwaway Edge capability probe. Only touches a new temporary profile + fixture.
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
const root = await mkdtemp(join(tmpdir(), 'cepm-edge-cdp-'));
const profile = join(root, 'profile'), extension = join(root, 'fixture');
await mkdir(extension);
await writeFile(join(extension, 'manifest.json'), JSON.stringify({manifest_version:3,name:'CEPM disposable probe',version:'1.0.0',background:{service_worker:'worker.js'},permissions:['storage']}));
await writeFile(join(extension, 'worker.js'), 'chrome.storage.local.set({probe: "cepm"});');
const report = {root, observations:[]};
let proc, ws, seq=0; const requests = new Map();
function call(method,params={}) { return new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>{requests.delete(id);reject(new Error(method+' timeout'));},10000);requests.set(id,r=>{clearTimeout(timer);resolve(r)});ws.send(JSON.stringify({id,method,params}));}); }
async function launch(extra=[]) {
  await rm(join(profile,'DevToolsActivePort'),{force:true});
  proc=spawn('/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',[`--user-data-dir=${profile}`,'--remote-debugging-port=0','--no-first-run','--no-default-browser-check',...extra,'about:blank'],{stdio:'ignore'});
  let port,path;
  for(let i=0;i<100;i++){try{[port,path]=(await readFile(join(profile,'DevToolsActivePort'),'utf8')).trim().split('\n');break;}catch{await delay(100)}}
  if(!port)throw Error('No debugger endpoint');
  ws=new WebSocket(`ws://127.0.0.1:${port}${path}`);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
  ws.onmessage=e=>{const r=JSON.parse(e.data);if(requests.has(r.id)){requests.get(r.id)(r);requests.delete(r.id)}};
  return port;
}
async function stop(){if(ws?.readyState===1)await call('Browser.close');for(let i=0;i<100&&proc.exitCode===null;i++)await delay(100);ws?.close();if(proc.exitCode===null)throw Error('Scratch Edge failed to close');}
async function observe(label,method,params={}){const response=await call(method,params);report.observations.push({label,response});return response;}
try {
  await launch();
  await observe('version','Browser.getVersion');
  await observe('list without unsafe flag','Extensions.getExtensions');
  let loaded=await observe('load without unsafe flag','Extensions.loadUnpacked',{path:extension});
  let flags=[];
  if(loaded.error){await stop();flags=['--enable-unsafe-extension-debugging'];await launch(flags);loaded=await observe('load with unsafe flag','Extensions.loadUnpacked',{path:extension});}
  if(loaded.error)throw Error('Load unavailable');
  const id=loaded.result.id;
  await observe('list after load','Extensions.getExtensions');
  await stop();await launch(flags);
  await observe('list after complete restart','Extensions.getExtensions');
  await observe('load again','Extensions.loadUnpacked',{path:extension});
  await observe('uninstall fixture','Extensions.uninstall',{id});
  await observe('list after uninstall','Extensions.getExtensions');
  await stop();await launch(flags);
  await observe('list after uninstall + restart','Extensions.getExtensions');
  await stop();
} catch(e) {report.error=String(e);try{await stop()}catch{}}
await writeFile(join(root,'report.json'),JSON.stringify(report,null,2));
console.log(JSON.stringify(report,null,2));
if(report.error)process.exitCode=1;
