// Place only a completed full receipt. The backend verifies media and maps the canvas rectangle.
function placementField(name,label,value,type='number'){
 const input=el('input',{id:`blender-place-${name}`,type,value:String(value),step:'any','aria-label':label});
 return {input,node:el('label',{},label,input)};
}
function placementNumber(input,label){if(!String(input.value).trim())throw Error(`Enter ${label}`);const v=Number(input.value);if(!Number.isFinite(v))throw Error(`Invalid ${label}`);return v;}
function placementValidateId(idValue){if(!/^[a-zA-Z0-9_-]+$/.test(idValue))throw Error('Placement ID must use letters, numbers, underscores or hyphens.');}
function placementValidateTiming(t,d,r,box,clipSeconds){
 if(t<0||d<=0)throw Error('Start must be nonnegative and duration positive.');
 if(r<=0||r>16)throw Error('Playback rate must be in (0,16].');
 if(box[2]<=0||box[3]<=0)throw Error('Placement rectangle width and height must be positive.');
 if(!Number.isFinite(clipSeconds)||clipSeconds<=0)throw Error('Completed receipt has no valid clip duration.');
 if(d>clipSeconds/r+1e-6)throw Error(`Clip covers ${(clipSeconds/r).toFixed(3)}s at this playback rate; shorten the duration or lower the rate.`);
}
function placementValidateReceipt(job,jobParameter){if(!job.job||(jobParameter==='blender_job'&&(!job.character||!job.track)))throw Error('Completed receipt is missing its job or source action identity.');}
function placementRequest(ctx){
 const job=ctx.getJob();
 if(!job||job.state!=='completed'||job.partial!==false)throw Error('Render a completed full action before placing it on the video timeline.');
 if(ctx.renderBusy)throw Error('Wait for the current render before placing.');
 placementValidateId(ctx.id.input.value);
 const t=placementNumber(ctx.start.input,'start'),d=placementNumber(ctx.duration.input,'duration'),r=placementNumber(ctx.rate.input,'playback rate'),box=ctx.rect.map(f=>placementNumber(f.input,'rectangle'));
 placementValidateTiming(t,d,r,box,job.clip_seconds);
 placementValidateReceipt(job,ctx.jobParameter);
 return {id:ctx.id.input.value,[ctx.jobParameter]:job.job,...(ctx.scene?{scene:ctx.scene.scene}:{}),start:t,duration:d,rect:box,playback_rate:r,...(job.character&&job.track?{character:job.character,track:job.track}:{})};
}
function placementConfirmed(result,params,jobParameter){
 return result.id===params.id&&result[jobParameter]===params[jobParameter]&&result.start===params.start&&result.duration===params.duration&&result.playback_rate===params.playback_rate&&JSON.stringify(result.canvas_rect)===JSON.stringify(params.rect)&&result.media&&result.backup&&Number.isInteger(result.track_index);
}
async function placementPlace(ctx){
 if(ctx.placing||!ctx.current())return;
 let params;try{params=placementRequest(ctx);}catch(error){ctx.message(String(error),true);return;}
 ctx.placing=true;ctx.update();ctx.message('Placing rendered action…');
 try{
  const result=await motionCommand('video.place_track',params);
  if(!ctx.current()||ctx.getJob()?.job!==params[ctx.jobParameter])return;
  ctx.readback.textContent=JSON.stringify(result,null,2);
  if(result.ok!==true)throw Error(result.error||'Placement command did not confirm success.');
  if(!placementConfirmed(result,params,ctx.jobParameter))throw Error('Placement returned incomplete or mismatched confirmation; inspect the result before retrying.');
  ctx.message(`Placed ${result.id} · ${params.scene?`scene ${params.scene}`:`start ${params.start}s`} · track ${result.track_index}.`);
 }catch(error){if(ctx.current()&&ctx.getJob()?.job===params[ctx.jobParameter])ctx.message(String(error),true);}
 finally{ctx.placing=false;if(ctx.current())ctx.update();}
}
function clipPlacementControls({project,getJob,current,jobParameter='blender_job'}){
 const ctx:any={project,getJob,current,jobParameter,scene:null,placing:false,renderBusy:false,lastJob:null,defaultId:'',sceneRevision:0,validationError:false};
 const field=placementField;
 const scenePicker=el('select',{id:'blender-place-scene','aria-label':'Destination scene'});
 const id=field('id','Placement ID','','text'),start=field('start','Start (absolute seconds)',0),duration=field('duration','Duration (seconds)',1),rate=field('rate','Playback rate',1);
 const rect=[field('x','X (video pixels)',0),field('y','Y (video pixels)',0),field('width','Width (video pixels)',0),field('height','Height (video pixels)',0)];
 const jobInfo=el('p',{class:'muted',id:'blender-place-job'});
 const status=el('p',{id:'blender-place-status',role:'status','aria-live':'polite',class:'muted'});
 const command=el('code',{id:'blender-place-command'});
 const readback=el('pre',{id:'blender-place-result','aria-label':'Placement result'});
 const place=el('button',{id:'blender-place-submit',class:'ghost','data-commands':'video.place_track',disabled:true},'Place in scene');
 const copy=el('button',{id:'blender-place-copy',class:'ghost',disabled:true},'Copy placement command');
 const inputs=[scenePicker,id.input,start.input,duration.input,rate.input,...rect.map(f=>f.input)];
 Object.assign(ctx,{id,start,duration,rate,rect,readback});
 const message=(text,error=false)=>{status.textContent=text;status.className=error?'image-error':'muted';};
 ctx.message=message;
 function request(){return placementRequest(ctx);}
 function update(){
  for(const input of inputs)input.disabled=ctx.placing||ctx.renderBusy;
  try{
   const params=request();
   command.textContent=magicCommandText('video.place_track',params,project);
   place.disabled=copy.disabled=ctx.placing;
   if(ctx.validationError){ctx.validationError=false;message('Ready to place. Start uses absolute project time.');}
  }catch(error){place.disabled=copy.disabled=true;command.textContent='';if(!ctx.placing){ctx.validationError=true;message(String(error),true);}}
 }
 ctx.update=update;
 function sceneId(){
  const tag=ctx.scene?`scene-${ctx.scene.scene}`:'timeline';
  const next=`${jobParameter==='blender_job'?'blender':'animation'}-${tag}-${getJob()?.track||getJob()?.backend||'action'}`.replace(/[^a-zA-Z0-9_-]/g,'-');
  if(!id.input.value||id.input.value===ctx.defaultId)id.input.value=next;
  ctx.defaultId=next;
 }
 function refresh(){
  const job=getJob();
  if((job?.job||null)!==ctx.lastJob){
   ctx.lastJob=job?.job||null;readback.textContent='';
   const canvas=job?.canvas;
   if(Array.isArray(canvas)&&canvas.length===2&&canvas.every(v=>Number.isFinite(v)&&v>0)){
    rect.forEach((f,i)=>{f.input.value=String([0,0,...canvas][i]);});
   }
   sceneId();
  }
  jobInfo.textContent=job?`${job.character||job.backend||'Native source'}${job.track?` · ${job.track}`:''} · ${job.job} · ${job.partial===false?'full action':'preview'}`:'No completed action selected.';
  update();
 }
 async function selectScene(){
  const revision=++ctx.sceneRevision;
  if(!scenePicker.value){
   ctx.scene=null;sceneId();message('Ready to place at any movable start time.');refresh();return;
  }
  const n=Number(scenePicker.value);ctx.scene=null;update();
  try{
   const value=await motionCommand('storyboard.read',{scene:n});
   if(!current()||revision!==ctx.sceneRevision)return;
   if(value.scene!==n||!Number.isFinite(value.start)||!Number.isFinite(value.end)||value.end<=value.start)throw Error('Scene has no valid authored timing.');
   ctx.scene=value;start.input.value=String(value.start);duration.input.value=String(value.end-value.start);sceneId();message('Ready to place. Start uses absolute project time.');refresh();
  }catch(error){if(current()&&revision===ctx.sceneRevision){message(String(error),true);place.disabled=copy.disabled=true;}}
 }
 async function load(){
  try{
   const toc=await motionCommand('storyboard.toc');if(!current())return;
   const chapters=(toc.chapters||[]).filter(s=>Number.isSafeInteger(s.scene)&&s.scene>0);
   scenePicker.replaceChildren(el('option',{value:''},'Movable timeline (no scene required)'),...chapters.map(s=>el('option',{value:String(s.scene)},s.title||`Scene ${s.scene}`)));
   if(!chapters.length){scenePicker.value='';await selectScene();return;}
   scenePicker.value=String(chapters[0].scene);await selectScene();
  }catch(error){if(current()){scenePicker.replaceChildren(el('option',{value:''},'Movable timeline (no scene required)'));scenePicker.value='';await selectScene();}}
 }
 scenePicker.addEventListener('change',selectScene);
 for(const input of inputs.filter(n=>n!==scenePicker))input.addEventListener('input',()=>{message('Ready to place.');update();});
 copy.onclick=async()=>{try{request();await navigator.clipboard.writeText(command.textContent);if(current())message('Exact placement command copied; nothing executed.');}catch(error){if(current())message(String(error),true);}};
 place.onclick=async()=>{await placementPlace(ctx);};
 const node=el('details',{id:'blender-placement'},el('summary',{id:'blender-placement-head'},el('span',{class:'panel-title'},'Place in scene')),
  el('div',{id:'blender-placement-body'},jobInfo,el('label',{},'Authored scene',scenePicker),id.node,
   el('p',{class:'muted'},'An existing placement with this ID is replaced. Rectangle frames the original canvas in video pixels; the backend maps the rendered frame.'),
   start.node,duration.node,rate.node,el('div',{id:'blender-place-coordinates'},...rect.map(f=>f.node)),place,status,
   el('details',{},el('summary',{},'Exact CLI command'),command,copy),el('details',{},el('summary',{},'Placement result'),readback)));
 refresh();
 return {node,load,refresh,setBusy(value){ctx.renderBusy=value;update();}};
}
