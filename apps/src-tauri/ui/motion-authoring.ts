// Persisted rig edits use shared commands and optimistic rig hashes, never preview-only transforms.
async function animationTheatreSnapshot(){
 theatreRequireSaved();
 if(magicTheatre.availability!=='ready'||!magicTheatre.ready||!magicTheatre.frame)throw Error(magicTheatre.error||'Wait for the native timeline before capturing a pose.');
 const project=state.project,frame=magicTheatre.frame,request=++magicTheatre.nextRequest;
 const value=await new Promise<any>((resolve,reject)=>{const timer=setTimeout(()=>{magicTheatre.exports.delete(request);reject(Error('Timeline snapshot timed out; no pose exported.'));},5000);magicTheatre.exports.set(request,{project,resolve,reject,timer});frame.contentWindow.postMessage({magicTheatre:true,type:'export',request,project},location.origin);});
 if(state.project!==project||magicTheatre.frame!==frame)throw Error('Project changed during timeline capture.');
 theatreRequireSaved();return value.state;
}
function buildAuthoringField(id: string, label: string, value: any, type = 'number', range: any[] | null = null): any {
 if(range&&typeof value==='number'&&Number.isFinite(value)){
  const [min,max,step,unit]=range,node=motionRange(`authoring-${id}`,label,Math.min(min,value),Math.max(max,value),step,value,unit,()=>{});
  const input=node.input;input.defaultValue=input.value;
  return {input,node};
 }
 const input=el('input',{id:`authoring-${id}`,class:'field',type:type==='number'?'text':type,inputmode:type==='number'?'decimal':undefined,value:value??'',placeholder:value===null?'Animated':undefined,'aria-label':label});
 return new InspectorField(label,input);
}

function buildPartCoordinateRows(meta: any, part: any, action: any, dirty: Set<string>) {
 const values: Record<string, any[]>={},partRows=[];
 for(const [property,label,count]of [['position','Position',2],['scale','Scale',2],['rotation','Rotation',1],['pivot','Pivot',2]] as [string,string,number][]){
  const v=property==='pivot'?part?.pivot:action?.static_overrides?.[part?.id]?.[property];
  const fields=Array.from({length:count},(_,i)=>{
   const size=meta.canvas?.[i],range=property==='scale'?[-200,200,1,'%']:property==='rotation'?[-180,180,1,'°']:Number.isFinite(size)&&size>0?[property==='pivot'?0:-size,size,0.1,'px']:null;
   return buildAuthoringField(count===1?property:`${property}-${i}`,count===1?label:`${label} ${i?'Y':'X'}`,count===1?v:v?.[i]??null,'number',range);
  });values[property]=fields;
  for(const f of fields)f.input.onchange=()=>dirty.add(property);
  partRows.push(el('div',{class:count===2?'motion-coordinate-row':''},...fields.map(f=>f.node)));
 }
 return {values,partRows};
}

function buildPartRequest(part: any, dirty: Set<string>, values: Record<string, any[]>, identity: () => Record<string, any>) {
 if(!part)throw Error('Select a part.');
 if(!dirty.size)throw Error('Change a part value first.');
 const p={...identity(),part:part.id};
 for(const k of dirty){
  const v=values[k].map(f=>{
   if(!String(f.input.value).trim()||!Number.isFinite(Number(f.input.value)))throw Error(`Enter every ${k} coordinate; animated channels need an explicit constant.`);
   return Number(f.input.value);
  });
  p[k]=v.length===1?v[0]:v;
 }
 return p;
}

function applyDurationChange(p: any, action: any, actionDirty: Set<string>, durationInput: any, placeDurationInput: any) {
 const d=Number(durationInput.value);
 if(!actionDirty.has('duration')&&!(Number.isFinite(d)&&d>0&&d!==action?.duration))return;
 if(!Number.isFinite(d)||d<=0||d>600)throw Error('Duration must be in (0,600].');
 p.duration=d;placeDurationInput.value=String(d);
}

function buildActionSetPayload(identity: () => Record<string, any>, action: any, actionDirty: Set<string>, labelInput: any, durationInput: any, loopInput: any, placeDurationInput: any) {
 const p=identity(),lbl=labelInput.value.trim(),curLabel=action?.label||action?.id||'';
 if(actionDirty.has('label')||(lbl&&lbl!==curLabel))p.label=lbl;
 applyDurationChange(p,action,actionDirty,durationInput,placeDurationInput);
 if(actionDirty.has('loop')||loopInput.checked!==Boolean(action?.loop))p.loop=loopInput.checked;
 if(Object.keys(p).length<=4)throw Error('Change an action value first.');
 return p;
}

function readPlaceRect(ctx: any) {
 const wDefault=ctx.meta.canvas?.[0]||1920,hDefault=ctx.meta.canvas?.[1]||1080;
 return [Number(ctx.placeX.input.value||0),Number(ctx.placeY.input.value||0),Number(ctx.placeW.input.value||wDefault),Number(ctx.placeH.input.value||hDefault)];
}

function validatePlaceVideoInputs(ctx: any) {
 const startVal=Number(ctx.placeStart.input.value);
 const durVal=Number(ctx.placeDuration.input.value||ctx.duration.input.value||ctx.action?.duration||3);
 const rateVal=Number(ctx.placeRate.input.value||1);
 if(!(startVal>=0)||!(durVal>0)||!(rateVal>0)){
  throw Error('Enter a valid non-negative start and positive duration/rate.');
 }
 const rect=readPlaceRect(ctx);
 const sceneNum=ctx.placeScene.value?Number(ctx.placeScene.value):undefined;
 const clipId=sceneNum?`${ctx.character}-scene-${sceneNum}-${ctx.cleanTrack}`.replace(/[^a-zA-Z0-9_-]/g,'-'):(ctx.existingPlacement?.id||ctx.defaultClipId);
 return {startVal,durVal,rateVal,rect,sceneNum,clipId};
}

async function runPlaceVideoTrack(ctx: any) {
 ctx.placeBtn.disabled=true;
 ctx.placeStatus.textContent='Exporting & syncing animation to Storyboard and HyperFrames Studio…';
 try{
  const {startVal,durVal,rateVal,rect,sceneNum,clipId}=validatePlaceVideoInputs(ctx);
  const exported=await motionCommand('animation.render',{character:ctx.character,track:motion.track,rest:motion.rest,amount:motion.amount,duration:durVal,sync_video:true});
  const videoFile=exported?.outputs?.[0]?.file;
  const placeParams={id:clipId,character:ctx.character,track:motion.track,start:startVal,duration:durVal,playback_rate:rateVal,rect,...(videoFile?{path:videoFile}:{}),...(sceneNum?{scene:sceneNum}:{})};
  const res=await motionCommand('video.place_track',placeParams);
  if(sceneNum){
   try{await motionCommand('storyboard.place_asset',{asset:ctx.character,page:sceneNum,x:100,y:100,width:220});}catch{}
  }
  if(!ctx.current())return;
  motion.lastPlacement=res;
  ctx.placeStatus.textContent=`Linked ${res.id} to${sceneNum?` Storyboard Scene ${sceneNum} &`:''} HyperFrames Studio track #${res.track_index} (${res.start}s–${(Number(res.start)+Number(res.duration)).toFixed(2)}s)`;
  await animationRefresh();
 }catch(error){if(ctx.current()){ctx.placeStatus.textContent=String(error);ctx.placeStatus.className='image-error';}}
 finally{if(ctx.current())ctx.placeBtn.disabled=false;}
}

function applyCapturedPoseReceipt(ctx: any, result: any) {
 if(result.ok!==true||!result.asset_id||!result.receipt||result.approval!=='pending')throw Error('Pose returned incomplete pending-asset receipt.');
 motion.lastPose={project:ctx.project,character:ctx.character,track:ctx.track,result};
 if(result.path){ctx.posePreview.src=convertFileSrc(result.path);ctx.posePreview.hidden=false;}
 ctx.poseResult.textContent=JSON.stringify(result,null,2);
 ctx.poseStatus.textContent='Pose registered in memory · pending approval. Save project explicitly to retain registration.';
}

async function runCapturePendingPose(ctx: any) {
 ctx.capture.disabled=true;
 try{
  if(motion.authoringBusy)throw Error('Wait for the current rig edit.');
  theatreRequireSaved();
  if(typeof animationPause==='function')animationPause();
  const params={...ctx.identity(),time:motion.time,rest:motion.rest===true,amount:motion.amount??100,...(ctx.reference.value?{reference_asset:ctx.reference.value}:{})};
  const snapshot=await animationTheatreSnapshot();
  if(!ctx.current())return;
  const result=await motionCommand('motion.pose.export',{...params,theatre_state:snapshot});
  if(!ctx.current())return;
  applyCapturedPoseReceipt(ctx,result);
  await animationRefresh();
 }catch(error){if(ctx.current())ctx.poseStatus.textContent=`${motion.lastPose?.result?'Pose receipt retained. ':''}${error}`;}
 finally{ctx.capture.disabled=ctx.caps.pose_export!==true;}
}

function validatePlacePoseInputs(ctx: any) {
 const pose=motion.lastPose;
 if(pose?.project!==ctx.project||!pose.result.asset_id)throw Error('Capture a pending pose in this project first.');
 const scene=Number(ctx.scenes.value);
 if(!Number.isSafeInteger(scene)||scene<=0)throw Error('Choose an authored destination scene.');
 const coords=[ctx.x,ctx.y,ctx.width].map(f=>Number(f.input.value));
 if(coords.some(v=>!Number.isFinite(v))||coords[2]<=0)throw Error('Enter finite coordinates and positive width.');
 return {pose,scene,coords};
}

async function runPlaceCapturedPose(ctx: any) {
 try{
  const {pose,scene,coords}=validatePlacePoseInputs(ctx);
  const target=await motionCommand('storyboard.read',{scene});
  if(!ctx.current())return;
  if(!Number.isSafeInteger(target.page)||target.page<=0)throw Error('Scene has no authored storyboard page.');
  const params={asset:pose.result.asset_id,page:target.page,x:coords[0],y:coords[1],width:coords[2]};
  const result=await motionCommand('storyboard.place_asset',params);
  if(!ctx.current())return;
  if(result.ok!==true||result.placement?.asset!==params.asset||!result.draft)throw Error('Placement returned no draft confirmation.');
  pose.placement=result;
  ctx.poseResult.textContent=JSON.stringify(pose,null,2);
  ctx.poseStatus.textContent='Pose placed in storyboard draft · still pending approval. Review and Save in Project.';
 }catch(error){if(ctx.current())ctx.poseStatus.textContent=String(error);}
}

function populatePlaceSceneOptions(placeScene: HTMLSelectElement, current: () => boolean, existingScene: any) {
 motionCommand('storyboard.toc').then(toc=>{
  if(!current())return;
  const chapters=(toc.chapters||[]).filter(s=>Number.isSafeInteger(s.scene)&&s.scene>0);
  for(const s of chapters){
   const opt=el('option',{value:String(s.scene)},`Scene ${s.scene}${s.title?` · ${s.title}`:''}${Number.isFinite(s.start)&&Number.isFinite(s.end)?` (${s.start.toFixed(1)}s–${s.end.toFixed(1)}s)`:''}`);
   placeScene.append(opt);
  }
  if(existingScene)placeScene.value=String(existingScene);
 }).catch(()=>{});
}

function createRigEditRunner(ctx: any) {
 return async function edit(id: string, params: any) {
  if(motion.authoringBusy)throw Error('Wait for the current rig edit.');
  theatreRequireSaved();if(!ctx.current())throw Error('Selected project or character changed.');
  motion.authoringBusy=true;for(const control of ctx.editControls)control.disabled=true;
  try{
   ctx.cli.textContent=magicCommandText(id,params,ctx.project);ctx.status.textContent='Applying…';
   const result=await motionCommand(id,params);
   if(ctx.current())ctx.readback.textContent=JSON.stringify(result,null,2);
   if(result.ok!==true||result.character!==ctx.character||!result.rig_sha256||!result.undo)throw Error('Edit returned no hash/undo confirmation; inspect result.');
   if(!ctx.current())return;
   if(result.track&&typeof result.track==='string')motion.track=result.track;
   motion.lastEdit={project:ctx.project,character:ctx.character,track:motion.track,command:id,result};
   localStorage.setItem(ctx.key,JSON.stringify({undo:result.undo,hash:result.rig_sha256}));
   ctx.readback.textContent=JSON.stringify(result,null,2);
   ctx.status.textContent='Rig saved · undo available · project Save remains explicit.';
   motion.authoringBusy=false;
   try{await animationRefresh();}catch(error){motion.lastEdit.refresh_error=String(error);ctx.status.textContent=`Rig edit succeeded; refresh failed: ${error}`;}
  }finally{motion.authoringBusy=false;for(const control of ctx.editControls)control.disabled=control.unavailable===true;}
 };
}

function buildActionPlacementSection(ctx: any) {
 const {meta,character,track,action,caps,identity,current,run,status}=ctx;
 const label=buildAuthoringField('label','Action name',action?.label||action?.id||'','text');
 const duration=buildAuthoringField('duration','Action duration (seconds)',action?.duration);
 const loop=el('input',{id:'authoring-loop',type:'checkbox','aria-label':'Loop authored action'});
 loop.checked=action?.loop===true;loop.defaultChecked=loop.checked;
 const actionDirty=new Set<string>();
 label.input.onchange=()=>actionDirty.add('label');duration.input.onchange=()=>actionDirty.add('duration');loop.onchange=()=>actionDirty.add('loop');
 const cleanTrack=(track||'action').replace(/^@/,'');
 const defaultClipId=`${character}-${cleanTrack}`.replace(/[^a-zA-Z0-9_-]/g,'-');
 const existingPlacement=(meta.placed_tracks||[]).find(p=>p.id===defaultClipId||(p.character===character&&String(p.track||'').replace(/^@/,'')===cleanTrack));
 const placeScene=el('select',{id:'authoring-place-scene','aria-label':'Storyboard scene'},el('option',{value:''},'Movable timeline (or pick a Storyboard scene…)'));
 const placeStart=buildAuthoringField('place-start','Video start (seconds)',existingPlacement?.start??0);
 const placeDuration=buildAuthoringField('place-duration','Video duration (seconds)',existingPlacement?.duration??action?.duration??3);
 const placeRate=buildAuthoringField('place-rate','Playback rate (×)',existingPlacement?.playback_rate??1);
 const defaultRect=Array.isArray(existingPlacement?.rect)&&existingPlacement.rect.length===4?existingPlacement.rect:[0,0,meta.canvas?.[0]||1920,meta.canvas?.[1]||1080];
 const placeX=buildAuthoringField('place-x','Stage X (px)',defaultRect[0]),placeY=buildAuthoringField('place-y','Stage Y (px)',defaultRect[1]);
 const placeW=buildAuthoringField('place-w','Stage W (px)',defaultRect[2]),placeH=buildAuthoringField('place-h','Stage H (px)',defaultRect[3]);
 const placeStatus=el('p',{id:'authoring-place-status',class:'muted',role:'status'},existingPlacement?`Synced on HyperFrames Studio track #${existingPlacement.track_index}${existingPlacement.scene?` · Storyboard Scene ${existingPlacement.scene}`:''} (${existingPlacement.start}s–${(Number(existingPlacement.start)+Number(existingPlacement.duration)).toFixed(2)}s · ${existingPlacement.playback_rate??1}×)`:'Choose a Storyboard scene or start time to link this animation to the Storyboard & HyperFrames Studio.');
 populatePlaceSceneOptions(placeScene,current,existingPlacement?.scene);
 placeScene.onchange=async()=>{
  if(!placeScene.value)return;
  try{
   const s=await motionCommand('storyboard.read',{scene:Number(placeScene.value)});
   if(current()&&Number.isFinite(s.start)&&Number.isFinite(s.end)&&s.end>s.start){
    placeStart.input.value=String(s.start);placeDuration.input.value=String(Number((s.end-s.start).toFixed(2)));
    placeStatus.textContent=`Scene ${s.scene} selected (${s.start.toFixed(2)}s–${s.end.toFixed(2)}s) · ready to link to Storyboard & HyperFrames Studio.`;
   }
  }catch{}
 };
 const placeBtn=el('button',{id:'authoring-place-video',class:'start','data-commands':'video.place_track',
  onclick:()=>runPlaceVideoTrack({meta,character,action,cleanTrack,defaultClipId,existingPlacement,placeScene,placeStart,placeDuration,placeRate,placeX,placeY,placeW,placeH,placeStatus,placeBtn,duration,current})},
  existingPlacement?'Update Storyboard & Studio Link':'Link to Storyboard & Studio');
 const actionApply=el('button',{id:'authoring-action-apply','data-commands':'motion.action.set',
  onclick:()=>{try{return run('motion.action.set',buildActionSetPayload(identity,action,actionDirty,label.input,duration.input,loop,placeDuration.input));}catch(error){status.textContent=String(error);}}},'Save action settings');
 actionApply.disabled=caps.action_set!==true;
 const actionNode=el('div',{class:'panel-fields'},...(caps.action_set===true?[label.node,duration.node,el('label',{},loop,'Loop authored action'),actionApply]:[]),el('hr',{}),el('strong',{},'Storyboard & HyperFrames Studio link'),el('label',{class:'field'},'Storyboard scene',placeScene),placeStart.node,placeDuration.node,placeRate.node,el('div',{class:'motion-coordinate-row'},placeX.node,placeY.node),el('div',{class:'motion-coordinate-row'},placeW.node,placeH.node),placeBtn,placeStatus);
 return {actionNode,actionApply,label,duration,loop};
}

function buildPoseAuthoringSection(ctx: any) {
 const {project,character,track,caps,identity,current,reason}=ctx;
 const posePreview=el('img',{id:'authoring-pose-preview',alt:'Captured static pose pending approval'});posePreview.hidden=true;
 const poseStatus=el('p',{id:'authoring-pose-status',role:'status'}),poseResult=el('pre',{id:'authoring-pose-result'});
 const reference=el('select',{id:'authoring-reference','aria-label':'Optional registered pose reference'},el('option',{value:''},'Source provenance only'),...(motion.assets||[]).filter(a=>a.kind==='image').map(a=>el('option',{value:a.id},a.name||a.id)));
 const capture=el('button',{id:'authoring-pose-export','data-commands':'motion.pose.export',
  onclick:()=>runCapturePendingPose({project,character,track,caps,identity,current,reference,posePreview,poseResult,poseStatus,capture})},'Capture pending pose');
 capture.disabled=caps.pose_export!==true;
 const scenes=el('select',{id:'authoring-pose-scene','aria-label':'Pose destination scene'});
 const x=buildAuthoringField('pose-x','Storyboard X',0),y=buildAuthoringField('pose-y','Storyboard Y',0),width=buildAuthoringField('pose-width','Storyboard width',160);
 const place=el('button',{id:'authoring-pose-place','data-commands':'storyboard.place_asset',
  onclick:()=>runPlaceCapturedPose({project,current,scenes,x,y,width,poseResult,poseStatus})},'Place pose in storyboard draft');
 const loadScenes=el('button',{id:'authoring-pose-scenes',onclick:async()=>{
  try{
   const toc=await motionCommand('storyboard.toc');if(!current())return;
   scenes.replaceChildren(...(toc.chapters||[]).filter(v=>Number.isSafeInteger(v.scene)&&v.scene>0).map(v=>el('option',{value:String(v.scene)},v.title||`Scene ${v.scene}`)));
   if(!scenes.options.length)throw Error('No authored storyboard scenes.');
  }catch(error){if(current())poseStatus.textContent=String(error);}
 }},'Load authored scenes');
 const poseNode=el('div',{class:'panel-fields'},reason('pose',caps.pose_export===true),reference,capture,poseStatus,posePreview,el('details',{},el('summary',{},'Place captured pose'),loadScenes,scenes,el('div',{class:'motion-coordinate-row'},x.node,y.node),width.node,place),poseResult);
 if(motion.lastPose?.project===project){
  poseResult.textContent=JSON.stringify(motion.lastPose,null,2);
  poseStatus.textContent='Captured pose · pending approval; project Save is explicit.';
  if(motion.lastPose.result.path){posePreview.src=convertFileSrc(motion.lastPose.result.path);posePreview.hidden=false;}
 }
 return {poseNode,capture};
}

function buildLayerStackSection(ctx: any) {
 const {meta,project,character,hash,part,key,undoEdit,run,status,editControls}=ctx;
 const order=meta.layer_order||meta.layers.map(l=>l.id),index=part?order.indexOf(part.id):-1,layerCaps=meta.layer_authoring||{};
 const layerPicker=el('select',{id:'authoring-layer-picker','aria-label':'Layer in stacking order',onchange:e=>{motion.selected=e.target.value||null;animationControls();animationDraw();updateMagicSelection();}},el('option',{value:''},'Choose a layer…'),...order.map((id,i)=>el('option',{value:id},`${i} · ${meta.layers.find(l=>l.id===id)?.label||id}`)));
 layerPicker.value=part?.id||'';
 const depth=buildAuthoringField('layer-depth','Stack depth',index<0?undefined:index,'number',[0,Math.max(0,order.length-1),1,'']);
 depth.input.min=0;depth.input.max=Math.max(0,order.length-1);depth.input.step=1;depth.input.disabled=!part||!layerCaps.supported;
 const layerReason=el('p',{id:'authoring-layer-reason'},layerCaps.supported?`0 = back · ${Math.max(0,order.length-1)} = front. Stacking preserves parents; path and scale control spatial depth.`:layerCaps.reason||'Stack editing requires a project-local ordered rig.');
 const move=value=>{if(!part||!Number.isInteger(value)||value<0||value>=order.length){status.textContent='Choose a layer and an integer depth within this stack.';return;}return run('motion.layers.set',{expected_project:project,character,expected_rig_sha256:hash,part:part.id,depth:value});};
 const back=el('button',{id:'authoring-layer-back',title:'Move selected layer one step toward the back',onclick:()=>move(index-1)},'Back');back.disabled=!layerCaps.supported||index<=0;
 const front=el('button',{id:'authoring-layer-front',title:'Move selected layer one step toward the front',onclick:()=>move(index+1)},'Front');front.disabled=!layerCaps.supported||index<0||index>=order.length-1;
 const layerApply=el('button',{id:'authoring-layer-apply','data-commands':'motion.layers.set',onclick:()=>move(String(depth.input.value).trim()?Number(depth.input.value):NaN)},'Apply depth');layerApply.disabled=!part||!layerCaps.supported;
 const layerUndo=el('button',{id:'authoring-layer-undo','data-commands':'motion.edit.undo',onclick:undoEdit},'Undo rig edit');layerUndo.disabled=!localStorage.getItem(key);
 editControls.push(depth.input,back,front,layerApply,layerUndo);
 return el('div',{class:'panel-fields'},...(layerCaps.supported?[layerPicker,layerReason,depth.node,el('div',{class:'motion-path-actions'},back,front,layerApply,layerUndo)]:[]));
}

class MotionAuthoringController {
  renderPanels(meta: any) {
    const project=state.project,character=motion.character,track=motion.track,part=meta.layers.find(l=>l.id===motion.selected),action=meta.tracks.find(t=>t.id===track),caps=action?.authoring||{},hash=meta.rig_sha256;
    const current=()=>state.project===project&&motion.character===character;
    const identity=(): Record<string, any>=>({expected_project:project,character,track,expected_rig_sha256:hash});
    const status=el('p',{id:'authoring-edit-status',role:'status'}),readback=el('pre',{id:'authoring-edit-result'}),cli=el('code',{id:'authoring-command'});
    const reason=(id,allowed)=>el('p',{id:`authoring-${id}-reason`,class:'muted'},allowed?'Changes replace only the edited action channels. Save project separately.':caps.reason||'A project-local authored action and authoring backend are required.');
    const editControls=[],key=`animation.editUndo.${project}.${character}`;
    const edit=createRigEditRunner({project,character,key,current,editControls,cli,status,readback});
    const run=(id,params)=>edit(id,params).catch(error=>{if(current()){status.textContent=String(error);status.className='image-error';}});
    const dirty=new Set<string>(),{values,partRows}=buildPartCoordinateRows(meta,part,action,dirty);
    const apply=el('button',{id:'authoring-part-apply','data-commands':'motion.part.set',onclick:()=>{try{return run('motion.part.set',buildPartRequest(part,dirty,values,identity));}catch(error){status.textContent=String(error);}}},'Apply part values');apply.disabled=!part||caps.part_set!==true;
    const reset=el('button',{id:'authoring-part-reset','data-commands':'motion.part.reset',onclick:()=>run('motion.part.reset',{...identity(),part:part.id,fields:['position','scale','rotation','pivot']})},'Reset part');reset.disabled=!part||caps.part_reset!==true;
    const undoEdit=()=>{try{const saved=JSON.parse(localStorage.getItem(key)||'null');if(!saved)throw Error('No saved undo receipt.');return run('motion.edit.undo',{...identity(),expected_rig_sha256:hash,undo:saved.undo});}catch(error){status.textContent=String(error);}};
    const undo=el('button',{id:'authoring-undo','data-commands':'motion.edit.undo',onclick:undoEdit},'Undo rig edit');undo.disabled=!localStorage.getItem(key);undo.title='Undo is checked against the current rig hash; Theatre edits can block it.';
    const partChildren=part&&caps.part_set===true?[el('strong',{},part.label||part.id),...partRows,apply,reset,undo,el('details',{},el('summary',{},'Edit receipt and exact CLI'),cli,readback)]:[el('p',{class:'muted'},part?'This layer can be previewed with motion. Saved layer controls appear when available.':'Choose a layer to edit its position, scale, rotation and pivot.')];
    const partNode=el('div',{class:'panel-fields'},...partChildren);
    if(motion.lastEdit?.project===project&&motion.lastEdit.character===character){readback.textContent=JSON.stringify(motion.lastEdit,null,2);status.textContent='Rig saved · undo receipt available · project Save remains explicit.';}
    const {actionNode,actionApply,label,duration,loop}=buildActionPlacementSection({meta,character,track,action,caps,identity,current,run,status});
    const layersNode=buildLayerStackSection({meta,project,character,hash,part,key,undoEdit,run,status,editControls});
    const {poseNode,capture}=buildPoseAuthoringSection({project,character,track,caps,identity,current,reason});
    editControls.push(apply,reset,undo,actionApply,capture,...Object.values(values).flat().map(f=>f.input),label.input,duration.input,loop);
    for(const control of editControls)control.unavailable=control.disabled===true;
    return {status,part:partNode,action:actionNode,pose:caps.pose_export===true?poseNode:null,layers:layersNode};
  }
}

const motionAuthoringController = new MotionAuthoringController();
function motionAuthoringPanels(meta: any){
  return motionAuthoringController.renderPanels(meta);
}
