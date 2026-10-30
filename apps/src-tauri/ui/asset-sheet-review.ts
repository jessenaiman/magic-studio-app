// A full source sheet remains a project asset. Reviewing it never extracts or replaces it.
class AssetSheetReview {
  acceptedReference: any;
  asset: any;
  busy: any;
  canvas: any;
  controls: any;
  existing: any;
  file: any;
  frameManifest: any;
  fullPrompt: any;
  guideToggle: any;
  guideVisible: any;
  image: any;
  info: any;
  kind: any;
  kinds: any;
  layoutDirty: any;
  layoutRevision: any;
  layoutUpdate: any;
  medium: any;
  mediumStatus: any;
  motion: any;
  motionField: any;
  observer: any;
  panels: any;
  plan: any;
  project: any;
  prompt: any;
  prompts: any;
  removeBackgroundButton: any;
  root: any;
  rowActions: any;
  rowMode: any;
  rowModeField: any;
  rowPicker: any;
  sheet: any;
  status: any;
  timing: any;
  constructor(asset, kinds = null, file = null) {
    this.project = state.project;
    this.asset = asset;
    this.kinds = kinds;
    this.file = file;
    this.prompts = new Map();
    this.layoutRevision = 0;
    this.busy = false;
  }
  async open() {
    $('asset-sheet-review')?.close();
    try {
      this.plan = await motionCommand('assets.sheet_plan', {asset:this.asset,kinds:[this.kinds?.[0]||'poses']});
      this.acceptedReference=this.plan.reference;
      let saved={};try{saved=JSON.parse(localStorage.getItem(this.draftKey())||'{}');}catch{}
      this.plan.sheets.forEach(sheet=>this.prompts.set(sheet.key,saved[sheet.key]||sheet.direction));
      if(state.project!==this.project)return;
      this.render();
      await this.show(this.file || this.plan.source || this.plan.reference);
    } catch(error) { toast(String(error)); }
  }
  draftKey() {return `magic.sheet.panel-directions.${this.project}.${this.asset}`;}
  savePrompt() {
    this.prompts.set(this.sheet.key,this.prompt.value);
    this.fullPrompt.value=`${this.prompt.value}\n\n${this.sheet.template_prompt}`;
    try{localStorage.setItem(this.draftKey(),JSON.stringify(Object.fromEntries(this.prompts)));}catch{}
  }
  render() {
    const close=el('button',{id:'asset-sheet-close',class:'icon',title:'Close sheet review','aria-label':'Close sheet review',html:'<svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18"/></svg>',onclick:()=>{if(!this.busy)this.root.close();}});
    const fit=el('button',{id:'asset-sheet-fit',class:'icon',title:'Fit complete sheet','aria-label':'Fit complete sheet',html:`<svg viewBox="0 0 24 24">${MOTION_ICONS.fit}</svg>`,onclick:()=>this.fit()});
    const actual=el('button',{id:'asset-sheet-actual',class:'ghost',title:'View original pixels at 100%','aria-label':'Sheet actual size',onclick:()=>this.zoom(1)},'1:1');
    this.guideToggle=el('button',{id:'asset-sheet-guide-toggle',class:'ghost','aria-label':'Show panel guide','aria-pressed':'false',onclick:()=>this.toggleGuide()},'Panel guide');
    this.image=el('img',{id:'asset-sheet-image',alt:'Reference artwork'});
    this.image.addEventListener('load',()=>{this.fit();this.actions();});
    this.image.addEventListener('error',()=>{this.status.textContent='Source image could not be loaded. No preparation has started.';this.info=null;this.actions();});
    this.canvas=el('div',{id:'asset-sheet-canvas',class:'artwork-review-canvas',tabindex:0,'aria-label':'Complete sheet · scroll at actual size'},this.image);
    this.status=el('p',{id:'asset-sheet-status',class:'muted',role:'status'});
    this.prompt=el('textarea',{id:'asset-sheet-prompt',class:'field',rows:4,'aria-label':'Editable drawing direction',oninput:()=>this.savePrompt()});
    this.kind=el('select',{id:'asset-sheet-kind','aria-label':'Sheet type',onchange:()=>this.selectTemplate()},
      this.plan.sheet_types.map(type=>el('option',{value:type.id},type.label)));
    this.kind.value=this.kinds?.[0]||'poses';
    this.motion=el('select',{id:'asset-sheet-motion','aria-label':'Action verb',onchange:()=>this.selectTemplate()},
      this.plan.motions.map(action=>el('option',{value:action.id},action.label)));
    this.motion.value=this.plan.default_motion;
    this.rowMode=el('input',{id:'asset-sheet-four-rows',type:'checkbox','aria-label':'Four action rows',onchange:()=>this.selectTemplate()});
    this.rowActions=this.plan.default_action_rows.map((action,index)=>{
      const select=el('select',{id:`asset-sheet-row-${index+1}`,'aria-label':`Action row ${index+1}`,onchange:()=>this.selectTemplate()},
        this.plan.motions.filter(m=>m.id!=='custom').map(m=>el('option',{value:m.id},m.label)));
      select.value=action;return select;
    });
    this.rowPicker=el('div',{id:'asset-sheet-action-rows'},...this.rowActions.map((select,i)=>new InspectorField(`Row ${i+1}`,select).node));
    this.rowModeField=el('label',{},this.rowMode,'Four action rows');
    this.panels=el('textarea',{id:'asset-sheet-panels',class:'field',rows:6,'aria-label':'Ordered panel descriptions',oninput:()=>{this.layoutDirty=true;this.actions();}});
    this.fullPrompt=el('textarea',{id:'asset-sheet-full-prompt',class:'field',rows:8,readonly:true,'aria-label':'Full Gaffer prompt with panel template'});
    this.timing=el('p',{id:'asset-sheet-timing',class:'muted'});
    this.motionField=new InspectorField('Action verb',this.motion).node;
    this.layoutUpdate=el('button',{id:'asset-sheet-update-layout',class:'ghost','data-commands':'assets.sheet_plan',onclick:()=>this.selectTemplate(true)},'Update panel guide');
    this.applyTemplate(this.plan.sheets[0]);
    this.existing=el('select',{id:'asset-sheet-source','aria-label':'Complete sheet to review',onchange:()=>this.show(this.existing.value)},
      el('option',{value:this.plan.source||this.plan.reference},'Reference artwork'),
      ...(this.plan.source&&this.plan.source!==this.plan.reference?[el('option',{value:this.plan.reference},'Original reference')]:[]),
      ...this.plan.sheets.filter(sheet=>sheet.exists).map(sheet=>el('option',{value:sheet.file},sheet.label)));
    if(this.file&&!([...this.existing.options].some(option=>option.value===this.file)))this.existing.append(el('option',{value:this.file},'Selected sheet'));
    this.controls=el('div',{id:'asset-sheet-actions',class:'transport'});
    this.medium=el('select',{id:'asset-sheet-medium','aria-label':'Generated art style',onchange:()=>this.selectTemplate(true)},
      this.plan.styles.map(style=>el('option',{value:style.id},style.label)));
    this.medium.value='original';
    this.removeBackgroundButton=el('button',{id:'asset-sheet-remove-background',class:'ghost','data-commands':'asset.make_transparent',onclick:()=>this.removeBackground()},'Remove background again');
    this.mediumStatus=el('p',{id:'asset-sheet-medium-status',class:'muted'},'Reference for Gaffer');
    const promptFields=el('div',{class:'panel-fields'},
      new InspectorField('Reference / generated sheet',this.existing).node,
      new InspectorField('Generated art style',this.medium).node,
      el('details',{},el('summary',{},'Reference repair'),this.removeBackgroundButton,this.mediumStatus),
      new InspectorField('Sheet type',this.kind).node,this.rowModeField,this.motionField,this.rowPicker,
      new InspectorField('Panels · one label | drawing direction per line',this.panels).node,this.layoutUpdate,this.timing,
      new InspectorField('Drawing direction',this.prompt).node,
      el('details',{},el('summary',{},'Full Gaffer prompt'),this.fullPrompt),
      ...(this.plan.source_prompt?[el('details',{},el('summary',{},'Original generation prompt'),el('textarea',{id:'asset-sheet-original-prompt',class:'field',rows:8,readonly:true,'aria-label':'Original generation prompt'},this.plan.source_prompt))]:[]),
      el('p',{class:'muted'},'Generate a whole sheet, review it, then prepare draft pieces. The source stays intact.'));
    this.root=el('dialog',{id:'asset-sheet-review',class:'editor-dialog artwork-review','aria-label':'Asset sheets · full source review'},
      new PanelHeader({id:'asset-sheet-head',title:'Asset sheets',actions:[this.guideToggle,fit,actual,close]}).element,
      el('div',{class:'artwork-review-layout'},this.canvas,promptFields),this.status,this.controls);
    this.root.addEventListener('cancel',event=>{if(this.busy)event.preventDefault();});
    this.root.addEventListener('close',()=>{this.observer?.disconnect();this.root.remove();});
    document.body.append(this.root);this.root.showModal();
    this.observer=new ResizeObserver(()=>this.fit());this.observer.observe(this.canvas);
  }
  async show(file) {
    this.file=file;this.info=null;this.image.removeAttribute('src');this.actions();
    this.existing.value=file;this.status.textContent='Loading complete source…';
    try {
      const info=await motionCommand('assets.sheet_info',{asset:this.asset,sheet:file});
      if(!this.root.open||file!==this.file)return;
      this.info=info;this.showPreview();
      const reference=[this.plan.source,this.plan.reference,this.acceptedReference].includes(file);
      this.status.textContent=`${info.width} × ${info.height} · ${reference?'Reference ready · choose the style and action for Gaffer':info.approved?'Sheet approved · preparation is a separate action':'Review the full sheet before extracting pieces'}`;
      this.actions();
    } catch(error) {if(file===this.file){this.status.textContent=String(error);this.actions();}}
  }
  fit() {
    if(!this.image.naturalWidth)return;
    this.zoom(Math.min(1,this.canvas.clientWidth/this.image.naturalWidth,this.canvas.clientHeight/this.image.naturalHeight));
  }
  zoom(value) {
    this.canvas.style.setProperty('--review-width',`${this.image.naturalWidth}px`);
    this.canvas.style.setProperty('--review-zoom',String(value));
  }
  templateParams(edited=false) {
    const rows=this.rowMode.checked&&this.kind.value==='poses';
    return {asset:this.asset,kinds:[this.kind.value],motion:this.motion.value,reference:this.acceptedReference,art_style:this.medium?.value||'original',
      ...(rows?{action_rows:this.rowActions.map(select=>select.value)}:{}),
      ...(!rows&&(edited||this.motion.value==='custom')?{panels:this.panels.value.split('\n').map(v=>v.trim()).filter(Boolean)}:{})};
  }
  applyTemplate(sheet) {
    this.sheet=sheet;this.layoutDirty=false;
    this.prompt.value=this.prompt.defaultValue=this.prompts.get(sheet.key)||sheet.direction;
    this.panels.value=this.panels.defaultValue=sheet.layout.panels.map(p=>`${p.label} | ${p.description}`).join('\n');
    this.fullPrompt.value=this.fullPrompt.defaultValue=`${this.prompt.value}\n\n${sheet.template_prompt}`;
    this.motionField.hidden=sheet.kind!=='poses'||this.rowMode.checked;
    this.rowModeField.hidden=sheet.kind!=='poses';
    this.rowPicker.hidden=sheet.kind!=='poses'||!this.rowMode.checked;
    this.panels.disabled=this.rowMode.checked&&sheet.kind==='poses';
    const layout=sheet.layout;
    this.timing.textContent=`${layout.panels.length} panels · ${layout.columns} × ${layout.rows} · ${layout.fps} fps · ${layout.action_rows?.length?'Each action keeps its own timing':layout.looping?'Loop':'Play once, hold last frame'}`;
    if(this.guideVisible)this.showPreview();
  }
  async selectTemplate(edited=false) {
    this.layoutDirty=true;this.actions();
    const revision=++this.layoutRevision;
    try {
      const plan=await motionCommand('assets.sheet_plan',this.templateParams(edited));
      if(revision!==this.layoutRevision||state.project!==this.project||!this.root.open)return;
      this.applyTemplate(plan.sheets[0]);this.actions();
    }catch(error){this.layoutDirty=true;this.status.textContent=String(error);this.actions();}
  }
  removeBackground() {return this.perform(async()=>{
    this.mediumStatus.textContent='Removing background · preserving fine detail…';
    const result=await motionCommand('asset.make_transparent',{asset:this.asset,source:this.plan.reference,attach:true});
    await motionCommand('project.save');
    if(state.project!==this.project||!this.root.open)return;
    this.plan.reference=result.image;this.plan.source=result.image;this.acceptedReference=result.image;
    this.existing.replaceChildren(el('option',{value:result.image},'Transparent reference'));
    this.mediumStatus.textContent='Transparent reference ready · review fine details';
    this.guideVisible=false;this.guideToggle.textContent='Panel guide';this.guideToggle.setAttribute('aria-pressed','false');
    await this.show(result.image);
  });}
  toggleGuide() {
    this.guideVisible=!this.guideVisible;
    this.guideToggle.textContent=this.guideVisible?'Source artwork':'Panel guide';
    this.guideToggle.setAttribute('aria-pressed',String(this.guideVisible));
    this.guideToggle.setAttribute('aria-label',this.guideVisible?'Show source artwork':'Show panel guide');
    this.showPreview();this.actions();
  }
  showPreview() {
    this.image.alt=this.guideVisible?'Numbered panel guide · layout reference':'Reference artwork';
    this.image.src=this.guideVisible?`data:image/svg+xml;charset=utf-8,${encodeURIComponent(this.sheet.template_svg)}`:convertFileSrc(this.file);
  }
  actions() {
    if(!this.controls)return;
    const reference=[this.plan.source,this.plan.reference,this.acceptedReference].includes(this.file);
    const loaded=!!this.info&&!reference&&!this.guideVisible&&this.image.complete&&this.image.naturalWidth>0;
    const generate=el('button',{id:'asset-sheet-generate',class:reference?'start':'ghost','data-commands':'assets.agent_sheets',onclick:()=>this.generate()},this.busy?'Working…':'Generate with Gaffer');generate.disabled=this.busy||this.layoutDirty;
    const action=this.info?.approved
      ? el('button',{id:'asset-sheet-extract',class:'start','data-commands':'asset.cut_sheet',onclick:()=>this.extract()},'Extract draft pieces')
      : el('button',{id:'asset-sheet-approve',class:'start','data-commands':'assets.sheet_review',onclick:()=>this.approve()},'Approve full sheet');
    action.disabled=!loaded||this.busy;
    this.controls.replaceChildren(generate,...(reference?[]:[action]));
    if(this.frameManifest?.file===this.file) {
      const play=el('button',{id:'asset-sheet-playback',class:'ghost','data-commands':'assets.sheet_playback',onclick:()=>this.playback()},'Preview animation');play.disabled=this.busy;this.controls.append(play);
    }
  }
  async perform(work) {
    if(this.busy||state.project!==this.project)return;
    this.busy=true;this.kind.disabled=true;this.existing.disabled=true;this.motion.disabled=true;this.panels.disabled=true;this.prompt.disabled=true;this.layoutUpdate.disabled=true;this.removeBackgroundButton.disabled=true;this.medium.disabled=true;this.actions();
    this.rowMode.disabled=true;this.rowActions.forEach(select=>select.disabled=true);
    try {await work();}catch(error){this.status.textContent=String(error);}
    finally {this.busy=false;this.kind.disabled=false;this.existing.disabled=false;this.motion.disabled=false;this.panels.disabled=this.rowMode.checked;this.prompt.disabled=false;this.layoutUpdate.disabled=false;this.removeBackgroundButton.disabled=false;this.medium.disabled=false;this.rowMode.disabled=false;this.rowActions.forEach(select=>select.disabled=false);this.actions();}
  }
  generate() {return this.perform(async()=>{
    const kind=this.kind.value;this.status.textContent='Drawing a complete sheet draft. Extraction will wait for your review.';
    const result=await motionCommand('assets.agent_sheets',{...this.templateParams(true),prompts:{[this.sheet.key]:this.prompt.value},model:$('image-model')?.value,run:true});
    await motionCommand('project.save');
    if(result.ok!==true)throw Error(result.agent_error||'The sheet draft was not completed. Existing source artwork is unchanged.');
    const sheet=result.sheets?.[0],file=sheet?.raw?.file||sheet?.file;
    if(!file)throw Error('No complete sheet was returned.');
    this.existing.append(el('option',{value:file},'New draft · review required'));
    const planned=this.plan.sheets.find(sheet=>sheet.kind===kind);if(planned)Object.assign(planned,{file,exists:true});
    this.guideVisible=false;this.guideToggle.textContent='Panel guide';this.guideToggle.setAttribute('aria-pressed','false');
    await this.show(file);
  });}
  approve() {return this.perform(async()=>{
    await motionCommand('assets.sheet_review',{expected_project:this.project,asset:this.asset,sheet:this.file,sha256:this.info.sha256,approved:true});
    await this.show(this.file);
  });}
  extract() {return this.perform(async()=>{
    const root=this.project.slice(0,this.project.lastIndexOf('/'));
    const result=await motionCommand('asset.cut_sheet',{asset:this.asset,source:this.file,kind:this.kind.value==='poses'?'action':this.kind.value==='turnaround'?'turnaround':'auto',out_dir:`${root}/assets/sheets/pieces/${this.asset}-${this.info.sha256.slice(0,12)}`});
    if(result.ok!==false&&result.animation)this.frameManifest={file:this.file,manifest:result.manifest};
    this.status.textContent=result.ok===false?'Draft extraction needs attention. The full source is preserved.':'Draft pieces prepared. Review them before building a rig.';
  });}
  playback() {return this.perform(async()=>{
    const result=await motionCommand('assets.sheet_playback',{manifest:this.frameManifest.manifest,id:`${this.asset}-frames-${this.info.sha256.slice(0,8)}`,name:`${this.asset} · action frames`});
    await motionCommand('project.save');
    this.root.close();await animationRefresh();
    motion.track='demonstration';motion.rest=false;motion.time=0;
    animationControls();await animationSample();await animationPlay();
  });}
}
function reviewAssetSheets(asset=motion.character,kinds=null,file=null) {
  if(asset)return new AssetSheetReview(asset,kinds,file).open();
}
