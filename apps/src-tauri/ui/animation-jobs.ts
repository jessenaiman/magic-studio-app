// Blender owns animation data; this surface only chooses jobs and plays rendered media.
let renderViewRevision=0,renderDialog=null;
let renderMediaUrl=null,renderMediaFetch=null,renderPollTimer=null;
function animationJobsDispose(){
 clearTimeout(renderPollTimer);renderPollTimer=null;
 ++renderViewRevision;renderMediaFetch?.abort();renderMediaFetch=null;
 const root=renderDialog;renderDialog=null;
 const video=root?.querySelector('video');
 if(video){video.pause();video.removeAttribute('src');video.load();}
 if(renderMediaUrl)URL.revokeObjectURL(renderMediaUrl);renderMediaUrl=null;
 if(root&&root.tagName==='DIALOG'){root.close();root.remove();}
}
async function loadRenderedClip(video,path,current){
 if(renderMediaUrl)URL.revokeObjectURL(renderMediaUrl);renderMediaUrl=null;
 renderMediaFetch?.abort();renderMediaFetch=new AbortController();
 const response=await fetch(convertFileSrc(path),{signal:renderMediaFetch.signal});
 if(!response.ok)throw new Error('Cannot read the rendered preview');
 const reader=response.body.getReader(),chunks=[];let size=0;
 while(true){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;
  if(size>64*1024*1024){await reader.cancel();throw new Error('Preview exceeds 64 MB; render a shorter or smaller preview');}chunks.push(value);}
 if(!current())return;
 renderMediaUrl=URL.createObjectURL(new Blob(chunks,{type:String(path).toLowerCase().endsWith('.mp4')?'video/mp4':'video/webm'}));
 video.src=renderMediaUrl;video.load();
}
function syncSelectTooltips(inputs: HTMLSelectElement[]) {
 for(const input of inputs){
  const tooltip=()=>{input.title=input.selectedOptions[0]?.textContent||'';};
  input.addEventListener('change',tooltip);tooltip();
 }
}
function trackLabelFor(t: any, character: string) {
 if(t.label)return t.label;
 if(character===motion.character){
  const match=motion.meta?.tracks?.find(a=>a.id===t.id);
  if(match?.label)return match.label;
 }
 return t.id.replace(/^@/,'').replaceAll('_',' ').replaceAll('-',' ');
}
function buildBlenderJobSelects(candidates: any[], meta: any, choice: any) {
 const picker=el('select',{'aria-label':'Animated object',id:'blender-object'},...candidates.map(a=>el('option',{value:a.id},a.name||a.id)));
 picker.value=choice.character;
 picker.onchange=()=>{motion.character=picker.value;openAnimationJobs({character:picker.value,track:choice.track});};
 const tracks=el('select',{'aria-label':'Animation action',id:'blender-track'},...meta.tracks.map(t=>el('option',{value:t.id,title:t.id},trackLabelFor(t,choice.character))));
 if([...tracks.options].some(option=>option.value===choice.track))tracks.value=choice.track;
 const source=el('select',{id:'blender-source','aria-label':'Animation source'},
  el('option',{value:'saved'},'Saved Blender scene'),el('option',{value:'legacy'},'Reimport legacy action'));
 const range=el('select',{id:'blender-range','aria-label':'Render length'},
  el('option',{value:'90'},'Preview · 90 frames max'),el('option',{value:'18000'},'Full · 18,000 frames max'));
 const resolution=el('select',{id:'blender-resolution','aria-label':'Render resolution'},
  ...[25,50,100].map(value=>el('option',{value:String(value)},`${value}% resolution`)));
 resolution.value='50';
 syncSelectTooltips([picker,tracks,source,range,resolution]);
 return {picker,tracks,source,range,resolution};
}
async function findFallbackCompletedJob(character: string, track: string, requestedJob: string, selected: () => boolean) {
 const saved=await motionCommand('animation.list');
 if(!selected())return null;
 const matching=(saved.jobs||[]).filter(j=>j.character===character&&j.track===track);
 const index=matching.findIndex(j=>j.job===requestedJob);
 for(const previous of index<0?[]:matching.slice(index+1)){
  const result=await motionCommand('animation.status',{job:previous.job});
  if(!selected())return null;
  if(result.state==='completed'&&result.blend)return result;
 }
 return null;
}
function createAnimationJobsPoller(ctx: any) {
 async function handleCompleted(value: any, selected: () => boolean) {
  ctx.setLatest(value);ctx.placement.refresh();
  ctx.source.options[0].disabled=false;ctx.source.value='saved';ctx.source.dispatchEvent(new Event('change'));
  await loadRenderedClip(ctx.video,value.clip,selected);if(!selected())return;
  ctx.details.textContent='Editable Blender scene saved';ctx.details.title=value.blend;
  ctx.report(value.partial?'Preview ready · shortened clip':'Preview ready');
 }
 async function handleFailedOrCancelled(value: any, requestedJob: string, selected: () => boolean) {
  const failure=value.state==='cancelled'?'Render cancelled':'Render failed';
  if(!ctx.getLatest()&&(value.state==='failed'||value.state==='cancelled')){
   const fallback=await findFallbackCompletedJob(ctx.choice.character,ctx.tracks.value,requestedJob,selected);
   if(!selected())return;
   if(fallback){
    ctx.setLatest(fallback);ctx.placement.refresh();
    ctx.source.options[0].disabled=false;ctx.source.value='saved';ctx.source.dispatchEvent(new Event('change'));
    ctx.details.textContent='Earlier successful Blender scene available';ctx.details.title=fallback.blend;
    await loadRenderedClip(ctx.video,fallback.clip,selected);if(!selected())return;
   }
  }
  const hasBlend=Boolean(ctx.getLatest()?.blend);
  ctx.report(`${failure}${hasBlend?' · previous saved scene available':''}`,
   value.state==='cancelled'?failure:`${failure} · ${value.error||value.detail||'Inspect the worker log'}`);
 }
 function reportActivePoll(value: any) {
  ctx.busy(true);ctx.cancel.disabled=value.state==='cancelling';
  ctx.report(value.state==='cancelling'?'Cancelling render…':value.progress?.total?`Rendering ${value.progress.completed} / ${value.progress.total}`:'Preparing animation…');
  renderPollTimer=setTimeout(poll,1000);
 }
 function handlePollError(error: any) {
  const state=ctx.getJobState(),active=!['completed','cancelled','failed'].includes(state);
  ctx.busy(active);ctx.cancel.disabled=!active||state==='cancelling';
  ctx.report(`${error}${active?' · Retrying status…':''}`);
  if(active)renderPollTimer=setTimeout(poll,1000);
 }
 async function poll() {
  const requestedJob=ctx.getJob();
  if(!ctx.current()||!requestedJob)return;
  const selectedRevision=ctx.getSelectionRevision();
  const selected=()=>ctx.current()&&ctx.getJob()===requestedJob&&ctx.getSelectionRevision()===selectedRevision;
  try{
   const value=await motionCommand('animation.status',{job:requestedJob});if(!selected())return;
   ctx.setJobState(value.state);
   if(!['completed','cancelled','failed'].includes(value.state))return reportActivePoll(value);
   ctx.busy(false);
   if(value.state==='completed')await handleCompleted(value,selected);
   else await handleFailedOrCancelled(value,requestedJob,selected);
  }catch(error){
   if(selected())handlePollError(error);
  }
 }
 return poll;
}
async function openAnimationJobs(selection: {character?: string, track?: string}={}){
 theatreRequireSaved();
 animationJobsDispose();
 const choice={character:selection.character??motion.character,track:selection.track??motion.track};
 const revision=++renderViewRevision,project=state.project,view=$('view-animation');
 view.replaceChildren();renderDialog=view;
 const current=()=>revision===renderViewRevision&&state.project===project&&state.view==='animation'&&motion.mode==='blender'&&renderDialog===view;
 const controls=el('aside',{class:'motion-inspector blender-controls',id:'blender-controls','aria-label':'Blender animation controls'});
 const stage=el('div',{class:'motion-stage'});
 const toolbar=new PanelHeader({id:'motion-stagebar',titleId:'blender-status',title:'Loading artwork…',actions:[motionModeSwitch()]});
 toolbar.label.setAttribute('role','status');
 view.append(controls,el('div',{class:'motion-main'},toolbar.element,stage));
 const video=el('video',{id:'blender-preview',controls:true,loop:true,'aria-label':'Rendered Blender animation'});
 stage.append(video);
 let latest=null,job=null,jobState=null,selectionRevision=0;
 const report=(message,detail=message)=>{if(current())toolbar.rename(message,detail);};
 try{
  const result=await motionCommand('project.list_assets');if(!current())return;
  const candidates=(result.assets||result.project_assets||[]).filter(a=>a.kind==='cutout_rig');
  if(!candidates.some(a=>a.id===choice.character))choice.character=candidates[0]?.id;
  if(!choice.character){report('Choose a rigged character in Project to begin.');return;}
  motion.character=choice.character;
  const meta=await motionCommand('animation.scene.actions',{character:choice.character});if(!current())return;
  const {picker,tracks,source,range,resolution}=buildBlenderJobSelects(candidates,meta,choice);
  controls.append(picker);
  if(!tracks.options.length){report('Author a motion track before migrating this rig.');return;}
  choice.track=tracks.value;motion.track=tracks.value;
  const render=el('button',{class:'start',id:'blender-render'},'Render preview');
  const cancel=el('button',{class:'ghost',id:'blender-cancel',disabled:true},'Cancel render');
  const details=el('p',{class:'muted'},'Editable Blender scene and transparent clip are saved with the project.');
  const placement=clipPlacementControls({project,getJob:()=>latest,current});
  const busy=value=>{placement.setBusy(value);render.disabled=value;cancel.disabled=!value;for(const input of [picker,tracks,source,range,resolution])input.disabled=value;};
  controls.append(el('label',{},'Action',tracks),el('label',{},'Source',source),
   el('label',{},'Length',range),el('label',{},'Resolution',resolution),render,cancel,details,placement.node);
  placement.load();
  const poll=createAnimationJobsPoller({current,choice,tracks,source,video,details,placement,cancel,busy,report,
   getLatest:()=>latest,setLatest:v=>{latest=v;},getJob:()=>job,getJobState:()=>jobState,setJobState:v=>{jobState=v;},getSelectionRevision:()=>selectionRevision});
  async function restore(){
   clearTimeout(renderPollTimer);const requestedSelection=++selectionRevision;
   renderMediaFetch?.abort();if(renderMediaUrl)URL.revokeObjectURL(renderMediaUrl);renderMediaUrl=null;
   job=null;jobState=null;latest=null;placement.refresh();busy(true);cancel.disabled=true;video.pause();video.removeAttribute('src');video.load();choice.track=tracks.value;
   const saved=await motionCommand('animation.list');if(!current()||requestedSelection!==selectionRevision)return;
   job=saved.jobs.find(j=>j.character===choice.character&&j.track===tracks.value)?.job;
   if(job){cancel.disabled=false;await poll();}else{busy(false);report('Ready to create an editable Blender animation');}
   if(!current()||requestedSelection!==selectionRevision)return;
   source.options[0].disabled=!latest?.blend;source.value=latest?.blend?'saved':'legacy';source.dispatchEvent(new Event('change'));
  }
  tracks.onchange=()=>restore().catch(error=>{if(current()){busy(false);report(String(error));}});
  render.onclick=async()=>{
   ++selectionRevision;clearTimeout(renderPollTimer);renderMediaFetch?.abort();
   busy(true);cancel.disabled=true;report('Submitting animation…');
   try{
    const value=await motionCommand('animation.scene.render',{character:choice.character,track:tracks.value,
     ...(source.value==='saved'&&latest?.blend?{blend:latest.blend}:{}),max_frames:Number(range.value),resolution_percent:Number(resolution.value)});
    if(!current())return;job=value.job;jobState=null;cancel.disabled=false;poll();
   }catch(error){if(!current())return;busy(false);report(String(error));}
  };
  cancel.onclick=async()=>{cancel.disabled=true;try{await motionCommand('animation.cancel',{job});if(current()){clearTimeout(renderPollTimer);await poll();}}catch(error){if(current())cancel.disabled=false;report(String(error));}};
  await restore().catch(error=>{if(current())busy(false);throw error;});
 }catch(error){report(String(error));}
}
