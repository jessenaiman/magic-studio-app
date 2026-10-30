// EffectCraft owns authoring; this panel previews animations configured through shared commands.
let effectcraftPreview=null;
function effectcraftAnimationDispose(){effectcraftPreview?.dispose();}
async function effectcraftAnimationOpen(){
 if(effectcraftPreview?.dialog.open)return;
 theatreRequireSaved();blenderDispose();
 effectcraftPreview=new EffectcraftPreview();
 await effectcraftPreview.load();
}
class EffectcraftPreview{
 constructor(){
  this.project=state.project;this.character=motion.character||null;this.track=motion.track;
  this.revision=0;this.sources=[];this.jobs=[];this.running=false;
  this.picker=el('select',{id:'effectcraft-animation','aria-label':'Animation',hidden:true,onchange:()=>this.select(this.picker.value).catch(error=>this.error(error))});
  this.video=el('video',{id:'effectcraft-preview',controls:true,loop:true,hidden:true,'aria-label':'EffectCraft animation preview'});
  this.status=el('p',{id:'effectcraft-status',class:'muted',role:'status','aria-live':'polite'},'Loading animation…');
  this.render=el('button',{id:'effectcraft-render',class:'start',disabled:true,'data-commands':'animation.source.render',onclick:()=>this.start()},'Render preview');
  this.cancel=el('button',{id:'effectcraft-cancel',class:'ghost',hidden:true,'data-commands':'animation.cancel',onclick:()=>this.cancelRender()},'Cancel render');
  this.ask=el('button',{id:'effectcraft-ask',class:'start',hidden:true,onclick:()=>{this.dispose();agentToggle(true);}},'Ask Sproket');
  const close=motionButton('effectcraft-close','Close EffectCraft preview','close',()=>this.dispose());
  this.dialog=el('dialog',{id:'effectcraft-preview-dialog',class:'editor-dialog effectcraft-preview-dialog','aria-label':'EffectCraft animation'},
   el('div',{class:'blender-render-header'},el('strong',{},'EffectCraft'),close),
   el('div',{class:'effectcraft-preview-body'},this.picker,this.video,this.status,el('div',{class:'effectcraft-preview-actions'},this.render,this.cancel,this.ask)));
  this.dialog.addEventListener('close',()=>this.dispose());
  document.body.append(this.dialog);this.dialog.showModal();
 }
 current(){return effectcraftPreview===this&&state.project===this.project&&state.view==='animation'&&(motion.character||null)===this.character;}
 dispose(){
  if(effectcraftPreview!==this)return;effectcraftPreview=null;clearTimeout(this.timer);
  blenderMediaFetch?.abort();if(blenderMediaUrl){URL.revokeObjectURL(blenderMediaUrl);blenderMediaUrl=null;}
  this.video.pause();this.video.removeAttribute('src');this.video.load();this.dialog.close();this.dialog.remove();
 }
 report(text,detail=text){if(this.current()){this.status.textContent=text;this.status.title=detail;}}
 error(error){this.report(String(error).split('\n')[0],String(error));}
 busy(value){this.running=value;this.render.disabled=value||!this.source;this.picker.disabled=value;this.cancel.hidden=!value;this.cancel.disabled=!value;}
 identity(){return {expected_project:this.project,id:this.source.id,expected_revision:this.source.revision};}
 async load(){
  try{
   const [links,jobs]=await Promise.all([motionCommand('animation.sources',{expected_project:this.project}),motionCommand('animation.list')]);
   if(!this.current())return;
   if(links.project!==this.project||!Array.isArray(links.sources))throw Error('Animation configuration changed; reopen EffectCraft.');
   this.sources=links.sources.filter(row=>row.backend==='effectcraft'&&(row.character||null)===this.character);this.jobs=jobs.jobs||[];
   this.picker.replaceChildren(...this.sources.map(row=>el('option',{value:row.id},motion.meta?.tracks?.find(track=>track.id===row.track)?.label||row.id.replaceAll('_',' ').replaceAll('-',' '))));
   this.picker.hidden=this.sources.length<2;
   if(!this.sources.length){this.render.hidden=true;this.ask.hidden=false;this.report('Ask Sproket to prepare an EffectCraft animation for this artwork.');return;}
   const selected=this.sources.find(row=>row.track===this.track)||this.sources[0];await this.select(selected.id);
  }catch(error){this.error(error);}
 }
 async select(id){
  clearTimeout(this.timer);const revision=++this.revision;
  this.source=this.sources.find(row=>row.id===id);this.picker.value=id;this.job=null;
  this.video.pause();this.video.removeAttribute('src');this.video.load();this.video.hidden=true;this.busy(false);
  const accepted=this.source.accepted;
  const recent=this.jobs.find(job=>job.backend==='effectcraft'&&job.source_id===id&&job.source_revision===this.source.revision);
  if(recent){this.job=recent.job;await this.poll(revision);}
  else if(accepted?.job){this.job=accepted.job;await this.poll(revision,accepted);}
  else this.report('Ready to render your animation.');
 }
 async poll(revision=this.revision,accepted=null){
  const job=this.job,selected=()=>this.current()&&revision===this.revision&&job===this.job;
  try{
   const result=await motionCommand('animation.status',{job});if(!selected())return;
   this.validate(result,accepted);
   if(!['completed','failed','cancelled'].includes(result.state)){
    this.busy(true);this.cancel.disabled=result.state==='cancelling';this.report(result.state==='cancelling'?'Cancelling render…':result.progress?.phase||'Rendering animation…');
    this.timer=setTimeout(()=>this.poll(revision),1000);return;
   }
   this.busy(false);await this.showResult(result,selected);
  }catch(error){if(selected()){this.busy(false);this.error(error);}}
 }
 validate(result,accepted){
  const expected={job:this.job,project:this.project,backend:'effectcraft',source_id:this.source.id,source_revision:this.source.revision-(accepted?1:0)};
  if(accepted)Object.assign(expected,{state:'completed',partial:false,clip:accepted.clip,clip_sha256:accepted.clip_sha256});
  if(!Object.entries(expected).every(([key,value])=>result[key]===value))throw Error('Animation changed; ask Sproket to refresh its configuration.');
 }
 async showResult(result,selected){
  if(result.state!=='completed'){this.report(result.state==='cancelled'?'Render cancelled.':'Rendering failed; ask Sproket to inspect the result.',result.error||result.state);return;}
  if(!result.clip)throw Error('The render has no preview; ask Sproket to inspect the result.');
  await blenderLoadPreview(this.video,result.clip,selected);if(!selected())return;
  this.video.hidden=false;this.report(result.partial?'Preview ready · shortened clip':'Preview ready');
 }
 async start(){
  if(!this.current()||this.running||!this.source)return;
  const revision=++this.revision;clearTimeout(this.timer);this.busy(true);this.cancel.disabled=true;this.report('Preparing animation…');
  try{
   const result=await motionCommand('animation.source.render',{...this.identity(),options:{}});
   if(!this.current()||revision!==this.revision)return;
   if(!result.job)throw Error('The render did not start; ask Sproket to inspect it.');
   this.job=result.job;await this.poll(revision);
  }catch(error){if(this.current()&&revision===this.revision){this.busy(false);this.error(error);}}
 }
 async cancelRender(){
  this.cancel.disabled=true;
  try{await motionCommand('animation.cancel',{job:this.job});if(this.current()){clearTimeout(this.timer);await this.poll();}}
  catch(error){if(this.current()){this.cancel.disabled=false;this.error(error);}}
 }
}
