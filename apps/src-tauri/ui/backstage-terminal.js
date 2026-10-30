// Navigation preserves this intentional single PTY session; it never starts one.
const backstageInvoke=window.__TAURI__.core.invoke;
const backstageTerminal={terminal:null,fit:null,running:false,busy:false,session:null,generation:0,closing:false,stopping:false,input:null,resize:null};
function backstageStatus(message){document.getElementById('backstage-status').textContent=message;}
function backstageControls(){
  document.getElementById('backstage-start').disabled=backstageTerminal.busy||backstageTerminal.running;
  document.getElementById('backstage-stop').disabled=backstageTerminal.busy||!backstageTerminal.running;
}
function backstageFit(){
  const host=document.getElementById('backstage-terminal');
  if(!backstageTerminal.fit||!host?.getBoundingClientRect().width||!host.getBoundingClientRect().height)return;
  const dimensions=backstageTerminal.fit.proposeDimensions();
  if(dimensions)backstageTerminal.terminal.resize(Math.max(2,Math.min(500,dimensions.cols)),Math.max(2,Math.min(200,dimensions.rows)));
}
function backstageResize(cols,rows){
  const session=backstageTerminal.session;
  if(!backstageTerminal.running||session===null)return;
  let request=backstageTerminal.resize;
  if(request?.session===session){request.latest={cols,rows};return;}
  request={session,latest:{cols,rows}};backstageTerminal.resize=request;
  (async()=>{
    while(request.latest&&backstageTerminal.running&&backstageTerminal.session===session){
      const dimensions=request.latest;request.latest=null;
      await backstageInvoke('backstage_resize',{session,...dimensions});
    }
  })().catch(error=>{
    if(backstageTerminal.running&&backstageTerminal.session===session)backstageStatus(`Terminal resize failed: ${error}`);
  }).finally(()=>{if(backstageTerminal.resize===request)backstageTerminal.resize=null;});
}
function backstageSend(data,binary=false){
  const queue=backstageTerminal.input,session=backstageTerminal.session;
  if(!backstageTerminal.running||session===null||!queue||queue.cancelled)return;
  if(data.length>65536){backstageStatus('Terminal input is too large; send at most 64 KiB at a time.');return;}
  const bytes=binary?Uint8Array.from(data,char=>char.charCodeAt(0)):new TextEncoder().encode(data);
  if(queue.pending+bytes.length>65536){backstageStatus('Terminal input is busy; this input was not queued.');return;}
  queue.pending+=bytes.length;
  queue.promise=queue.promise.then(async()=>{
    for(let offset=0;offset<bytes.length;offset+=8192){
      if(queue.cancelled||!backstageTerminal.running||backstageTerminal.session!==session)return;
      await backstageInvoke('backstage_write',{session,bytes:Array.from(bytes.slice(offset,offset+8192))});
    }
  }).catch(error=>{
    if(!queue.cancelled&&backstageTerminal.session===session)backstageStatus(`Terminal input failed: ${error}`);
  }).finally(()=>{queue.pending-=bytes.length;});
}
async function backstageRecover(){
  ++backstageTerminal.generation;
  if(backstageTerminal.input)backstageTerminal.input.cancelled=true;
  backstageTerminal.busy=true;backstageControls();
  try{
    const session=await backstageInvoke('backstage_current');
    backstageTerminal.session=session;backstageTerminal.running=session!==null;
    if(session!==null){
      backstageStatus('A previous Backstage session is closing or still running. Stop it before launching a new terminal.');
    }
  }catch(error){backstageStatus(`Could not inspect Backstage session: ${error}`);}
  finally{backstageTerminal.busy=false;backstageControls();}
}
async function backstageStart(){
  if(backstageTerminal.busy||backstageTerminal.running)return;
  backstageTerminal.busy=true;backstageControls();backstageStatus('Launching Backstage…');
  const generation=++backstageTerminal.generation;
  backstageTerminal.session=null;
  backstageTerminal.input={promise:Promise.resolve(),pending:0,cancelled:false};
  let ended=false,terminalError=null;
  try{
    if(!window.MagicBackstage)throw new Error('Terminal assets unavailable. Rebuild the desktop frontend.');
    if(!backstageTerminal.terminal){
      const {Terminal,FitAddon}=window.MagicBackstage;
      const theme=getComputedStyle(document.documentElement);
      const terminal=new Terminal({fontSize:13,scrollback:1000,screenReaderMode:true,cursorBlink:false,
        theme:{background:theme.getPropertyValue('--bg').trim(),foreground:theme.getPropertyValue('--text').trim(),cursor:theme.getPropertyValue('--accent').trim()}});
      const fit=new FitAddon();terminal.loadAddon(fit);terminal.open(document.getElementById('backstage-terminal'));
      backstageTerminal.terminal=terminal;backstageTerminal.fit=fit;
      terminal.onData(data=>backstageSend(data));
      terminal.onBinary(data=>backstageSend(data,true));
      terminal.onResize(({cols,rows})=>backstageResize(cols,rows));
      terminal.attachCustomKeyEventHandler(event=>!(event.key==='Tab'&&event.shiftKey));
      new ResizeObserver(backstageFit).observe(document.getElementById('backstage-terminal'));
    }
    const terminal=backstageTerminal.terminal;
    terminal.reset();backstageFit();
    const output=new window.__TAURI__.core.Channel();
    output.onmessage=message=>{
      if(generation!==backstageTerminal.generation)return;
      if(message.kind==='data'){
        if(!ended&&!backstageTerminal.closing&&!backstageTerminal.stopping){backstageTerminal.session=message.session;backstageTerminal.running=true;}
        terminal.write(new Uint8Array(message.bytes),()=>{
          backstageInvoke('backstage_ack',{session:message.session,sequence:message.sequence}).catch(error=>{
            if(generation===backstageTerminal.generation&&backstageTerminal.running)backstageStatus(`Terminal output acknowledgement failed: ${error}`);
          });
        });
      }else if(message.kind==='exit'){
        ended=true;backstageTerminal.running=false;backstageTerminal.session=null;
        backstageTerminal.input.cancelled=true;
        backstageStatus(terminalError||message.error||`Backstage exited · ${message.success?'normal exit':`status ${message.code}`}. Launch again when needed.`);backstageControls();
      }else if(message.kind==='error'){terminalError=message.message;backstageStatus(message.message);}
    };
    const session=await backstageInvoke('backstage_start',{cols:terminal.cols,rows:terminal.rows,output});
    if(backstageTerminal.closing){await backstageInvoke('backstage_stop',{session});return;}
    if(!ended){
      backstageTerminal.session=session;backstageTerminal.running=true;
      backstageStatus('Backstage running · q quits · Shift+Tab returns to desktop controls. Navigation preserves this session.');
      backstageFit();backstageResize(terminal.cols,terminal.rows);terminal.focus();
    }
  }catch(error){
    backstageTerminal.running=false;backstageTerminal.input.cancelled=true;
    backstageStatus(`Could not launch Backstage: ${error}`);
    // A page reload or another in-flight launch may already own the singleton.
    // Recover its identity so Stop remains available instead of trapping the user.
    await backstageRecover();
  }finally{backstageTerminal.busy=false;backstageControls();}
}
async function backstageStop(){
  if(backstageTerminal.busy||!backstageTerminal.running||backstageTerminal.session===null)return;
  const session=backstageTerminal.session;
  backstageTerminal.busy=true;backstageTerminal.running=false;backstageTerminal.stopping=true;
  if(backstageTerminal.input)backstageTerminal.input.cancelled=true;
  backstageControls();backstageStatus('Stopping Backstage…');
  try{
    // Stop never waits behind keyboard/paste delivery.
    await backstageInvoke('backstage_stop',{session});
    ++backstageTerminal.generation;
    backstageTerminal.session=null;backstageTerminal.running=false;backstageStatus('Backstage stopped. Launch again when needed.');
  }catch(error){backstageStatus(`Could not stop Backstage: ${error}`);await backstageRecover();}
  finally{backstageTerminal.stopping=false;backstageTerminal.busy=false;backstageControls();}
}
function backstageTerminalMount(){
  queueMicrotask(backstageRecover);
  const controls=window.el('div',{class:'backstage-controls'},
    window.el('button',{id:'backstage-start',class:'start',disabled:true,onclick:backstageStart},'Launch Backstage'),
    window.el('button',{id:'backstage-stop',class:'ghost',disabled:true,onclick:backstageStop},'Stop'));
  const header=new PanelHeader({id:null,title:'Backstage · live terminal',actions:[controls]}).element;
  header.classList.add('panel-head');header.dataset.look='heading';
  return window.el('section',{id:'backstage-panel',class:'panel','aria-label':'Interactive Backstage terminal'},header,
    window.el('p',{id:'backstage-status',class:'backstage-status',role:'status'},'Ready to launch. Backstage observes local production data; it never starts agents or models.'),
    window.el('div',{id:'backstage-terminal',class:'backstage-terminal','aria-label':'Backstage terminal'}));
}
window.addEventListener('backstage-visible',()=>requestAnimationFrame(backstageFit));
window.addEventListener('pagehide',()=>{
  backstageTerminal.closing=true;
  if(backstageTerminal.input)backstageTerminal.input.cancelled=true;
  const session=backstageTerminal.session;
  if(session!==null)backstageInvoke('backstage_stop',{session}).catch(()=>{});
});

window.addEventListener('pageshow',()=>{
  if(backstageTerminal.closing){backstageTerminal.closing=false;if(document.getElementById('backstage-panel'))backstageRecover();}
});

window.magicBackstageTerminal={mount:backstageTerminalMount};
