// Native Animation workspace: full-timeline inspection and reversible skeletal corrections.
class AnimationReviewController {
  active=false; project:string|null=null; stage:any=null; data:any=null;
  character=''; clip='idle'; time=0; playing=false; raf=0; draft:any=null;
  dirty=false; parent:number|null=null; saving=false; generation=0; compare=false;
  history(){return (this.data?.review.revisions||[]).filter((r:any)=>r.character===this.character&&r.clip===this.clip);}
  feedback(){return {active:this.active,character:this.character,clip:this.clip,time:this.time,playing:this.playing,dirty:this.dirty,revision:this.parent,status:this.draft?.status,corrections:this.draft?.keys?.length,issues:this.draft?.issues?.length};}
  canLeave(){if(!this.dirty&&!this.saving)return true;toast(this.saving?'Wait for the revision to finish saving':'Save or discard your animation revision before leaving');return false;}
  dispose(){this.pause();this.generation++;this.stage?.dispose();this.stage=null;this.active=false;}
  async open(){
    if(!theatreCanLeave())return;
    this.dispose();animationPause();tripo3DStage.dispose();motion.resizeObserver?.disconnect();
    this.active=true;this.project=state.project;const generation=this.generation;
    const view=$('view-animation');view.classList.add('rig-review');view.replaceChildren(el('p',{class:'muted'},'Loading animation review…'));
    try{
      this.data=await motionCommand('animation.review');if(generation!==this.generation||this.project!==state.project)return;
      if(!this.data.models.length){view.replaceChildren(el('p',{},'This project has no GLB characters.'),el('button',{onclick:()=>this.back()},'Back to artwork'));return;}
      if(!this.data.models.some((m:any)=>m.id===this.character))this.character=this.data.models[0].id;
      this.shell();this.stage=new (window as any).RigReviewStage($('review-canvas'));
      await this.stage.load(this.data.models.map((m:any)=>({...m,url:String(convertFileSrc(m.path))+'?v='+m.version})));
      if(generation!==this.generation)return;
      const names=[...new Set(this.data.models.flatMap((m:any)=>m.clips))] as string[];
      $('review-clip').replaceChildren(...names.map(n=>el('option',{value:n},n)));
      if(!names.includes(this.clip))this.clip=names.includes('idle')?'idle':names[0]||'';
      $('review-clip').value=this.clip;this.loadDraft();
    }catch(e){if(generation===this.generation){$('review-status').textContent=String(e);toast(String(e));}}
  }
  back(){if(!this.canLeave())return;this.dispose();$('view-animation').classList.remove('rig-review');motion.project=null;animationView.open(true);}
  shell(){
    const select=(id:string,label:string,options:any[],change:any)=>el('label',{},label,el('select',{id,'aria-label':label,onchange:change},options));
    const modelOptions=this.data.models.map((m:any)=>el('option',{value:m.id},m.name+(m.rigged?'':' · needs rigging')));
    const target=select('review-character','Character',modelOptions,(e:any)=>{if(!this.canLeave()){e.target.value=this.character;return;}this.pause();this.character=e.target.value;this.loadDraft();});
    const clips=select('review-clip','Animation',[],(e:any)=>{if(!this.canLeave()){e.target.value=this.clip;return;}this.pause();this.clip=e.target.value;this.loadDraft();});
    const focus=select('review-focus','View',[el('option',{value:'all'},'All characters'),...this.data.models.map((m:any)=>el('option',{value:m.id},m.name))],(e:any)=>{this.stage.focus=e.target.value;this.labels();this.draw();});
    const zoom=select('review-zoom','Zoom',[.75,1,1.5,2,3].map(v=>el('option',{value:v},`${v}×`)),(e:any)=>{this.stage.zoom=Number(e.target.value);this.draw();});
    const toggle=(id:string,label:string,key:string,value=false)=>el('label',{class:'review-check'},el('input',{id,type:'checkbox',checked:value,onchange:(e:any)=>{this.stage[key]=e.target.checked;this.draw();}}),label);
    const header=el('div',{class:'review-toolbar'},el('strong',{},'Full animation review'),target,clips,focus,zoom,toggle('review-skeleton','Skeleton','skeleton'),toggle('review-wireframe','Wireframe','wireframe'),toggle('review-grid','Grid','grid',true),el('button',{class:'ghost',id:'review-back',onclick:()=>this.back()},'Artwork tools'));
    const time=el('input',{id:'review-time',type:'number',min:0,step:1/30,value:0,'aria-label':'Source time',onchange:(e:any)=>this.seek(Number(e.target.value))});
    const transport=el('div',{class:'review-transport'},
      el('button',{id:'review-start',onclick:()=>this.seek(this.draft.trim_start)},'Start'),
      el('button',{id:'review-prev',onclick:()=>this.seek(this.time-1/30),'aria-label':'Previous frame'},'−1 frame'),
      el('button',{id:'review-play',onclick:()=>this.play()},'Play full clip'),
      el('button',{id:'review-next',onclick:()=>this.seek(this.time+1/30),'aria-label':'Next frame'},'+1 frame'),
      el('input',{id:'review-scrub',type:'range',min:0,max:1,step:1/30,value:0,'aria-label':'Animation timeline',oninput:(e:any)=>this.seek(Number(e.target.value))}),time,el('output',{id:'review-readout'},'0 / 0 s'));
    const sidebar=el('aside',{class:'review-inspector',id:'review-inspector','aria-label':'Animation revisions'});
    $('view-animation').replaceChildren(header,el('div',{class:'review-workspace'},el('section',{class:'review-stage'},el('div',{id:'review-canvas','aria-label':'Front and side character previews'}),el('div',{id:'review-labels','aria-hidden':'true'})),sidebar),transport,el('div',{class:'review-status',id:'review-status',role:'status'},'Loading models…'));
    $('review-character').value=this.character;$('review-zoom').value='1';
  }
  latest(id:string){return [...this.data.review.revisions].reverse().find((r:any)=>r.character===id&&r.clip===this.clip);}
  defaults(id:string){return {trim_start:0,trim_end:this.stage.duration(id)||this.stage.duration()||1,speed:1,loop:false,in_place:true,keys:[],issues:[],notes:'',status:'needs-visual-review'};}
  loadDraft(revision?:any){
    this.pause();this.stage.select(this.clip);
    const saved=revision||this.latest(this.character);
    this.draft=structuredClone(saved?.draft||this.defaults(this.character));this.parent=saved?.id||null;
    this.dirty=false;this.compare=false;this.time=this.draft.trim_start;
    $('review-scrub').max=this.stage.duration();$('review-time').max=this.stage.duration();
    this.inspector();this.labels();this.draw();
  }
  changed(){this.dirty=true;this.draft.status='needs-visual-review';if($('review-approval'))$('review-approval').value=this.draft.status;this.compare=false;if($('review-compare'))$('review-compare').checked=false;this.draw();}
  number(id:string,label:string,value:number,min:number,max:number,step:number,change:any){return el('label',{},label,el('input',{id,type:'number',min,max,step,value,'aria-label':label,onchange:(e:any)=>{const n=Number(e.target.value);if(!Number.isFinite(n)||n<min||n>max){e.target.value=value;return;}change(n);}}));}
  inspector(){
    const d=this.draft,hasClip=this.data.models.find((m:any)=>m.id===this.character)?.clips.includes(this.clip);
    const save=el('button',{id:'review-save',class:'start',disabled:!hasClip,onclick:()=>this.save()},'Save revision');
    const discard=el('button',{id:'review-discard',class:'ghost',onclick:()=>this.loadDraft()},'Discard edits');
    const history=el('select',{id:'review-history','aria-label':'Saved revisions',onchange:(e:any)=>{if(!this.canLeave()){e.target.value=this.parent||'';return;}const r=this.history().find((r:any)=>r.id===Number(e.target.value));this.loadDraft(r||{draft:this.defaults(this.character)});}},el('option',{value:''},'Original / new draft'),...this.history().map((r:any)=>el('option',{value:r.id},`Revision ${r.id} · ${new Date(r.created_unix_ms).toLocaleString()}`)));
    history.value=String(this.parent||'');
    const start=this.number('review-trim-start','Trim start (source seconds)',d.trim_start,0,3600,1/30,(n:number)=>{if(n>=d.trim_end){toast('Start must be before end');return;}d.trim_start=n;this.seek(n);this.changed();});
    const end=this.number('review-trim-end','Trim end (source seconds)',d.trim_end,0,this.stage.duration(this.character)||3600,1/30,(n:number)=>{if(n<=d.trim_start){toast('End must be after start');return;}d.trim_end=n;this.changed();});
    const speed=this.number('review-speed','Speed',d.speed,.1,4,.05,(n:number)=>{d.speed=n;this.changed();});
    const check=(id:string,label:string,key:string)=>el('label',{class:'review-check'},el('input',{id,type:'checkbox',checked:d[key],onchange:(e:any)=>{d[key]=e.target.checked;this.changed();}}),label);
    const compare=el('label',{class:'review-check'},el('input',{id:'review-compare',type:'checkbox',onchange:(e:any)=>{this.compare=e.target.checked;this.draw();}}),'Compare original pose');
    const bones=this.stage.bones(this.character);
    const bone=el('select',{id:'review-bone','aria-label':'Bone to adjust'},...bones.map((b:string)=>el('option',{value:b},b.replace('mixamorig',''))));
    const defaultBone=bones.find((b:string)=>/LeftArm$/.test(b));if(defaultBone)bone.value=defaultBone;
    const angle=(axis:string)=>this.number('review-angle-'+axis,axis.toUpperCase()+' correction (degrees)',0,-180,180,1,()=>{});
    const addKey=el('button',{id:'review-key',disabled:!hasClip||!bones.length,onclick:()=>{
      this.pause();const time=Math.round(this.time*30)/30,rotation=['x','y','z'].map(a=>Number($('review-angle-'+a).value));
      d.keys=d.keys.filter((k:any)=>!(k.bone===bone.value&&Math.abs(k.time-time)<1/60));d.keys.push({bone:bone.value,time,rotation});this.changed();this.inspector();
    }},'Set correction at this frame');
    const keys=el('div',{class:'review-list'},...d.keys.map((k:any,i:number)=>el('div',{},el('button',{class:'ghost',onclick:()=>this.seek(k.time)},`${k.bone.replace('mixamorig','')} · ${k.time.toFixed(3)} s · ${k.rotation.join(', ')}°`),el('button',{'aria-label':`Remove correction ${i+1}`,onclick:()=>{d.keys.splice(i,1);this.changed();this.inspector();}},'×'))));
    const issue=el('textarea',{id:'review-issue',rows:2,placeholder:'Describe a defect at the current frame…','aria-label':'Issue at current frame'});
    const addIssue=el('button',{id:'review-add-issue',disabled:!hasClip,onclick:()=>{if(!issue.value.trim())return;d.issues.push({time:Math.round(this.time*30)/30,text:issue.value.trim()});this.changed();d.status='issues-found';this.inspector();}},'Record issue at this frame');
    const issues=el('div',{class:'review-list'},...d.issues.map((n:any,i:number)=>el('div',{},el('button',{class:'ghost',onclick:()=>this.seek(n.time)},`${n.time.toFixed(3)} s · ${n.text}`),el('button',{'aria-label':`Remove issue ${i+1}`,onclick:()=>{d.issues.splice(i,1);this.changed();this.inspector();}},'×'))));
    const notes=el('textarea',{id:'review-notes',rows:3,'aria-label':'Revision notes',oninput:(e:any)=>{d.notes=e.target.value;this.changed();}});notes.value=d.notes;
    const approval=el('select',{id:'review-approval','aria-label':'Visual review status',onchange:(e:any)=>{d.status=e.target.value;this.dirty=true;this.draw();}},...['needs-visual-review','issues-found','approved'].map(v=>el('option',{value:v},v.replaceAll('-',' '))));approval.value=d.status;
    const section=(title:string,...body:any[])=>el('section',{class:'review-section'},el('h3',{},title),...body);
    $('review-inspector').replaceChildren(
      section('Revision',el('p',{class:'muted'},hasClip?'Changes apply to this character and clip.':'This character has no selected clip. Rigging or animation is still needed.'),history,el('div',{class:'review-actions'},save,discard),approval),
      section('Timing',start,end,speed,check('review-loop','Loop preview','loop'),check('review-in-place','Keep preview in place','in_place'),compare),
      section('Bone corrections',bone,angle('x'),angle('y'),angle('z'),addKey,el('small',{class:'muted'},'Offsets blend between correction keys and hold at the ends. Add zero-angle keys before and after a local fix.'),keys),
      section('Issues',issue,addIssue,issues),section('Notes',notes),
      section('Export',el('button',{id:'review-export',disabled:!hasClip,onclick:()=>this.export()},'Export revised GLB'),el('small',{class:'muted'},'Exports this clip at 30 fps with timing and bone corrections baked in. In-place is a preview aid; root motion is preserved.')));
  }
  labels(){
    const active=this.data.models.filter((m:any)=>this.stage.focus==='all'||m.id===this.stage.focus);
    $('review-labels').style.gridTemplateColumns=active.length===1?'1fr':'repeat(2,1fr)';
    $('review-labels').style.gridTemplateRows=`repeat(${Math.ceil(active.length/(active.length===1?1:2))},1fr)`;
    $('review-labels').replaceChildren(...active.map((m:any)=>el('div',{},el('strong',{},m.name),el('small',{},m.clips.includes(this.clip)?'Front | Side':'No animation · needs rigging / clip'),el('span',{},m.id===this.character?'Editing this character':'Saved pose / original'))));
  }
  draw(){
    if(!this.stage||!this.draft)return;
    const drafts:Record<string,any>={};for(const m of this.data.models)drafts[m.id]=m.id===this.character?this.draft:(this.latest(m.id)?.draft||this.defaults(m.id));
    if(this.compare)drafts[this.character]={...this.draft,keys:[]};
    this.stage.seek(this.time,drafts);
    $('review-time').value=this.time.toFixed(3);$('review-scrub').value=String(this.time);
    $('review-readout').textContent=`${this.time.toFixed(3)} / ${this.stage.duration().toFixed(3)} s · frame ${Math.round(this.time*30)}`;
    $('review-status').textContent=`${this.dirty?'Unsaved changes':this.parent?'Saved revision '+this.parent:'Original / unsaved draft'} · ${this.draft.status.replaceAll('-',' ')} · ${this.playing?'PLAYING':this.time>=this.draft.trim_end?'COMPLETE':'PAUSED'}${this.compare?' · Original pose comparison':''}`;
  }
  seek(time:number){this.pause();if(!Number.isFinite(time))return;this.time=Math.max(0,Math.min(this.stage.duration(),time));this.draw();}
  pause(){this.playing=false;cancelAnimationFrame(this.raf);if($('review-play'))$('review-play').textContent='Play full clip';}
  play(){if(this.playing){this.pause();this.draw();return;}if(!this.stage.duration())return;
    if(this.time>=this.draft.trim_end||this.time<this.draft.trim_start)this.time=this.draft.trim_start;
    this.playing=true;$('review-play').textContent='Pause';let previous=performance.now();
    const tick=(now:number)=>{if(!this.playing)return;this.time+=(now-previous)/1000*this.draft.speed;previous=now;
      if(this.time>=this.draft.trim_end){if(this.draft.loop)this.time=this.draft.trim_start+(this.time-this.draft.trim_start)%(this.draft.trim_end-this.draft.trim_start);else{this.time=this.draft.trim_end;this.pause();}}
      this.draw();if(this.playing)this.raf=requestAnimationFrame(tick);};this.raf=requestAnimationFrame(tick);
  }
  async save(){
    if(this.saving)return false;this.pause();this.saving=true;$('review-save').disabled=true;
    try{const result=await motionCommand('animation.save_revision',{expected_project:this.project,expected_revision:this.data.review.revision,character:this.character,clip:this.clip,parent:this.parent,draft:structuredClone(this.draft)});
      this.data.review.revision=result.revision;this.data.review.revisions.push(result.entry);this.parent=result.revision;this.dirty=false;this.inspector();this.draw();toast('Animation revision saved');return true;
    }catch(e){toast(String(e));return false;}finally{this.saving=false;if($('review-save'))$('review-save').disabled=false;}
  }
  async export(){
    this.pause();if(this.dirty||!this.parent){if(!await this.save())return;}
    const button=$('review-export');button.disabled=true;
    try{const bytes=await this.stage.exportClip(this.character,this.draft);const path=await invoke('save_animation_export',new Uint8Array(bytes),{headers:{'x-project':encodeURIComponent(this.project||''),'x-character':encodeURIComponent(this.character),'x-clip':encodeURIComponent(this.clip),'x-revision':String(this.parent)}});toast('Export saved: '+path);$('review-status').textContent='Export saved: '+path;}
    catch(e){toast(String(e));}finally{button.disabled=false;}
  }
}
const animationReview=new AnimationReviewController();
window.addEventListener('beforeunload',(e)=>{if(animationReview.active&&animationReview.dirty){e.preventDefault();e.returnValue='';}});
