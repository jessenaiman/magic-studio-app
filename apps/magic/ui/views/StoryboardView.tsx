import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { executeCommand, fileUrl, pickFile, ProjectAsset, StoryboardData, StoryboardPage, StoryboardSlot } from "../api";
import "../storyboard.css";

interface Props {projectPath:string;storyboard?:StoryboardData;onRefresh:()=>Promise<void>;onToast:(message:string)=>void}

const isPlate = (asset: ProjectAsset) => {
  const path = (asset.path || "").replace(/\\/g, "/").toLowerCase();
  const name = (asset.name || asset.id || "").toLowerCase();
  return path.includes("/backgrounds/") || path.includes("/plates/") || /\b(background|backdrop|plate|world|clearing|river)\b/.test(name);
};

// Stage Assets & Animate (moved here from the Project view): characters and scene plates
// staged for the storyboard. Cards keep their original actions.
const StageAssets: React.FC<{ onToast: (message: string) => void }> = ({ onToast }) => {
  const [assets, setAssets] = useState<ProjectAsset[]>([]);
  const [tab, setTab] = useState<"characters" | "plates">("characters");
  const load = useCallback(async () => {
    try {
      const snapshot = await invoke<{ data?: { assets?: { assets?: ProjectAsset[] } } }>("session_snapshot");
      setAssets(snapshot.data?.assets?.assets ?? []);
    } catch (cause) {
      onToast(String(cause));
    }
  }, [onToast]);
  useEffect(() => {
    void load();
  }, [load]);
  const plates = assets.filter(isPlate);
  const characters = assets.filter((asset) => !isPlate(asset));
  const visible = tab === "plates" ? plates : characters;
  const subtitle = (asset: ProjectAsset) =>
    isPlate(asset) ? "Scene Plate · Storyboard →" : asset.kind === "model_3d" || /\.(glb|gltf)$/i.test(asset.path || "") ? "3D Rig · Animate →" : "Cutout · Animate →";
  const run = async (fn: () => Promise<unknown>, refresh = false) => {
    try {
      await fn();
      if (refresh) await load();
    } catch (cause) {
      onToast(String(cause));
    }
  };
  return (
    <section id="stage-assets-panel" className="sb-stage" aria-label="Stage Assets & Animate">
      <div className="sb-stage-bar">
        <strong>Stage Assets &amp; Animate <span className="muted">({assets.length})</span></strong>
        <button id="stage-tab-characters" className={tab === "characters" ? "start" : "ghost"} onClick={() => setTab("characters")}>Characters / Animate ({characters.length})</button>
        <button id="stage-tab-plates" className={tab === "plates" ? "start" : "ghost"} onClick={() => setTab("plates")}>Backgrounds &amp; Plates ({plates.length})</button>
        <button id="stage-add-asset" className="ghost sb-stage-add" onClick={() => void run(async () => {
          const selected = await pickFile({ title: "Add Asset", extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg", "glb", "gltf"] });
          if (typeof selected === "string" && selected) {
            // The engine decides the kind (image or 3D model) from the file; `image` names the
            // assets/images/<name>/ folder an outside image is copied into.
            const name = (selected.split(/[/\\]/).pop() || selected).replace(/\.[^.]+$/, "");
            const added = await executeCommand<{ asset_id: string }>("project.add_asset", { path: selected, name, image: name, load: false });
            onToast(`Added ${added.asset_id}`);
          }
        }, true)}>+ Add Asset</button>
      </div>
      <div className="sb-stage-row">
        {visible.map((asset) => (
          <div
            key={asset.id}
            className="sb-stage-card"
            role="button"
            tabIndex={0}
            title={asset.name || asset.id}
            onClick={() => void run(async () => {
              await executeCommand("character.open", { id: asset.id });
              await executeCommand("ui.set", { view: "animation" });
            })}
          >
            <button
              id={`stage-asset-remove-${asset.id}`}
              className="ghost sb-stage-remove"
              aria-label={`Remove ${asset.name || asset.id}`}
              title={`Remove ${asset.name || asset.id} from project`}
              onClick={(event) => {
                event.stopPropagation();
                void run(() => executeCommand("project.remove_asset", { id: asset.id }), true);
              }}
            >×</button>
            {/\.(png|jpe?g|webp|gif|svg)$/i.test(asset.path || "")
              ? <img className="sb-stage-thumb" src={fileUrl(asset.path)} alt="" />
              : <div className="muted sb-stage-placeholder">{asset.kind === "model_3d" ? "3D RIG" : "PART"}</div>}
            <div className="sb-stage-meta">
              <div className="sb-stage-name">{asset.name || asset.id}</div>
              <div className="muted sb-stage-sub">{subtitle(asset)}</div>
            </div>
          </div>
        ))}
        {!visible.length && <p className="muted">{tab === "plates" ? "No background plates registered yet." : "No character cutouts registered yet."}</p>}
      </div>
    </section>
  );
};

/** The mark beside a slot: its state from `storyboard.check`, drawn by `.sb-status-*`. */
function StatusMark({status,issues}:{status:string;issues:number}){
  const glyph=issues?"⚠":status==="incomplete"?"●":status==="empty"?"○":"✓";
  return <span className={`sb-status sb-status-${status}`} aria-label={status==="empty"?"Empty visual; no required-field finding":status}>{glyph}</span>;
}

/** The text area or image chooser for one slot, shared by the page and the inspector. */
function SlotEditor({slot,value,busy,dirty,onChange,onSave,onCancel,onChoose,idPrefix}:{slot:StoryboardSlot;value:string;busy:boolean;dirty:boolean;onChange:(value:string)=>void;onSave:()=>void;onCancel:()=>void;onChoose:()=>void;idPrefix:string}){
  return <div className="sb-field-editor">
    {slot.kind==="image"
      ?<><input id={`${idPrefix}-field-${slot.label}`} aria-label={slot.display} value={value} onChange={event=>onChange(event.target.value)}/><button id={`${idPrefix}-choose-${slot.label}`} className="ghost" onClick={onChoose}>Choose image…</button></>
      :<textarea id={`${idPrefix}-field-${slot.label}`} aria-label={slot.display} rows={4} value={value} onChange={event=>onChange(event.target.value)}/>}
    <div><button id={`${idPrefix}-save-${slot.label}`} className="start" disabled={busy||!dirty} onClick={onSave}>Save field</button><button id={`${idPrefix}-cancel-${slot.label}`} className="ghost" disabled={busy} onClick={onCancel}>Cancel</button></div>
  </div>;
}

export const StoryboardView:React.FC<Props>=({projectPath,storyboard,onRefresh,onToast})=>{
  const [pages,setPages]=useState<StoryboardPage[]>([]),[check,setCheck]=useState<StoryboardData>(storyboard||{}),[pageIndex,setPageIndex]=useState(0);
  const [rail,setRail]=useState<string[]>([]),[preview,setPreview]=useState(""),[busy,setBusy]=useState(""),[error,setError]=useState(""),[zoom,setZoom]=useState(100);
  const [drafts,setDrafts]=useState<Record<string,string>>({}),[editing,setEditing]=useState<string|null>(null);
  const request=useRef(0),liveProject=useRef(projectPath);liveProject.current=projectPath;
  const folder=projectPath.replace(/[/\\][^/\\]+$/,"");
  const page=pages[Math.min(pageIndex,Math.max(0,pages.length-1))], report=check.pages?.find(item=>item.page===page?.page);
  const slotKey=(slot:StoryboardSlot)=>`${page?.page}:${slot.label}`;
  const slots=(page?.slots||[]).filter(slot=>slot.editable);
  const slotState=(slot:StoryboardSlot)=>report?.slots?.[slot.label]??{status:"complete",issues:[]};
  const load=async()=>{
    const current=++request.current,project=projectPath;setError("");
    try{
      const data=await executeCommand<{pages:StoryboardPage[]}>("storyboard.pages",{_expected_project:project});
      const completion=await executeCommand<StoryboardData>("storyboard.check",{_expected_project:project});
      const thumbs=await executeCommand<{files:string[];modified:number}>("storyboard.thumbnail",{rail:true,_expected_project:project});
      if(current!==request.current||liveProject.current!==project)return;
      setPages(data.pages);setCheck(completion);setRail(thumbs.files.map(file=>`${fileUrl(file)}?v=${thumbs.modified}`));
      setPageIndex(index=>Math.min(index,Math.max(0,data.pages.length-1)));
    }catch(cause){if(current===request.current){setError(String(cause));setPreview("");}}
  };
  useEffect(()=>{setPageIndex(0);setDrafts({});setEditing(null);void load();return()=>{++request.current;};},[projectPath]);
  useEffect(()=>{let cancelled=false;setPreview("");if(page){
    void executeCommand<{out:string;modified:number}>("storyboard.thumbnail",{page:page.page,scale:.7,_expected_project:projectPath})
      .then(shot=>{if(!cancelled)setPreview(`${fileUrl(shot.out)}?v=${shot.modified}`);})
      .catch(cause=>{if(!cancelled)setError(String(cause));});}return()=>{cancelled=true;};},[page,projectPath]);
  const action=async(name:string,run:()=>Promise<unknown>,refresh=false)=>{if(busy)return;setBusy(name);setError("");try{await run();if(refresh){await load();await onRefresh();}}catch(cause){setError(String(cause));onToast(String(cause));}finally{setBusy("");}};
  const copyLink=(index:number,label?:string)=>void action("Copying link…",async()=>{
    const link=await executeCommand<{text:string}>("storyboard.link",{page:index,...(label?{label}:{}),_expected_project:projectPath});
    await writeText(link.text);onToast(`Copied chat link for ${label??`page ${index+1}`}`);
  });
  const change=(slot:StoryboardSlot,value:string)=>{const key=slotKey(slot);setDrafts(old=>{const next={...old};if(value===(slot.text??slot.image??""))delete next[key];else next[key]=value;return next;});};
  const cancel=(slot:StoryboardSlot)=>{const key=slotKey(slot);setDrafts(old=>{const next={...old};delete next[key];return next;});setEditing(null);};
  const choose=(slot:StoryboardSlot)=>void pickFile({title:`Choose ${slot.display}`,extensions:["png","jpg","jpeg","webp","svg"]}).then(path=>{if(typeof path==="string"&&liveProject.current===projectPath)change(slot,path);}).catch(cause=>setError(String(cause)));
  const save=async(slot:StoryboardSlot)=>{const key=slotKey(slot),value=drafts[key];if(value===undefined||!page)return;
    await action("Saving field…",async()=>{await executeCommand("storyboard.set",{page:page.page,[slot.kind==="image"?"images":"fields"]:{[slot.label]:value},_expected_project:projectPath});setDrafts(old=>{const next={...old};delete next[key];return next;});setEditing(null);},true);
  };
  const editor=(slot:StoryboardSlot,idPrefix:string)=>{const key=slotKey(slot);return <SlotEditor slot={slot} idPrefix={idPrefix} value={drafts[key]??slot.text??slot.image??""} busy={!!busy} dirty={drafts[key]!==undefined}
    onChange={value=>change(slot,value)} onSave={()=>void save(slot)} onCancel={()=>cancel(slot)} onChoose={()=>choose(slot)}/>;};
  const [vx,vy,vx1,vy1]=page?.bounds??[0,0,1,1];
  return <div id="view-storyboard" className="storyboard-layout" data-document-dirty={Object.keys(drafts).length>0} data-document-saving={busy==="Saving field…"} data-document-label="Storyboard fields">
    <StageAssets onToast={onToast} />
    <aside className="sb-panel sb-index"><header><h2>Storyboard &amp; design index</h2><p>DesignCraft · {pages.length} pages</p></header>
      <button id="storyboard-refresh" className="ghost" disabled={!!busy} onClick={()=>void action("Refreshing…",async()=>{await load();await onRefresh();})}>Sync &amp; refresh</button>
      <nav aria-label="Storyboard pages">{pages.map((item,index)=>{const status=check.pages?.find(p=>p.page===item.page)?.status||"incomplete";return <button id={`sb-nav-${item.page+1}`} key={item.page} className={`sb-page-card ${page?.page===item.page?"selected":""}`} aria-current={page?.page===item.page?"page":undefined} title="Click to open · Right-click to copy a chat link" onClick={()=>{setPageIndex(index);setEditing(null);}} onContextMenu={event=>{event.preventDefault();copyLink(item.page);}}>
        <div className="sb-nav-preview">{rail[item.page]?<img src={rail[item.page]} alt={`Page ${item.page+1}: ${item.title}`}/>:<span>{item.page+1}</span>}</div><span><StatusMark status={status} issues={0}/> {item.page+1}. {item.title}</span></button>;})}</nav>
      <button id="storyboard-export" className="ghost" disabled={!!busy||!pages.length} onClick={()=>void action("Exporting PDF…",async()=>{await executeCommand("storyboard.export",{pdf:`${folder}/storyboard.pdf`,_expected_project:projectPath});onToast("Saved storyboard.pdf");})}>Export PDF</button>
    </aside>
    <main className="sb-panel sb-main"><header className="sb-toolbar"><button id="sb-prev" className="ghost" disabled={pageIndex===0} onClick={()=>{setPageIndex(i=>i-1);setEditing(null);}}>←</button><strong>{page?.title??"DesignCraft storyboard"}</strong><button id="sb-next" className="ghost" disabled={!pages.length||pageIndex>=pages.length-1} onClick={()=>{setPageIndex(i=>i+1);setEditing(null);}}>→</button>
      <label>Zoom <select id="sb-zoom" value={zoom} onChange={event=>setZoom(Number(event.target.value))}><option value={100}>Fit</option><option value={150}>150%</option><option value={200}>200%</option></select></label>
      <button id="storyboard-open-editor" className="ghost" disabled={!!busy||!pages.length} onClick={()=>void action("Opening DesignCraft…",()=>executeCommand("storyboard.open",{_expected_project:projectPath}))}>Open in DesignCraft ↗</button></header>
      <div id="sb-cards" className={`sb-spread ${zoom===100?"fit":"zoomed"}`}>{preview&&page?<div className={`sb-spread-stage sb-zoom-${zoom}`}>
        <img src={preview} alt={page.title}/>
        <svg className="sb-overlay" viewBox={`${vx} ${vy} ${vx1-vx} ${vy1-vy}`} preserveAspectRatio="none">
          {slots.map(slot=>{
            const b=slot.bounds;if(!b||b[2]<=b[0]||b[3]<=b[1])return null;
            const key=slotKey(slot),active=editing===key,state=slotState(slot);
            return <g key={slot.label}>
              <rect id={`sb-slot-${slot.label}`} className={`sb-slot sb-slot-${slot.kind} sb-status-${state.status} ${active?"active":""}`} x={b[0]} y={b[1]} width={b[2]-b[0]} height={b[3]-b[1]}
                role="button" tabIndex={0} aria-label={`${slot.display}: click to edit, right-click to copy a chat link`}
                onClick={()=>setEditing(active?null:key)} onKeyDown={event=>{if(event.key==="Enter")setEditing(active?null:key);}}
                onContextMenu={event=>{event.preventDefault();copyLink(page.page,slot.label);}}><title>{slot.display}</title></rect>
              {active&&<foreignObject className="sb-canvas-editor" x={b[0]} y={b[1]} width={Math.max(b[2]-b[0],420)} height={Math.max(b[3]-b[1],260)}>{editor(slot,"sb-canvas")}</foreignObject>}
            </g>;
          })}
        </svg>
      </div>:<p>{pages.length?"Rendering saved DesignCraft page…":"No readable DesignCraft storyboard yet."}</p>}</div>
      {error&&<p role="alert" className="sb-error">{error}</p>}<footer role="status">{busy||`${Object.keys(drafts).length?`${Object.keys(drafts).length} unsaved field(s) · `:""}Click a frame on the page or a field on the right to edit it · Right-click to copy a chat link.`}</footer>
      {!pages.length&&<button id="storyboard-new" className="start" disabled={!!busy} onClick={()=>void action("Creating storyboard…",()=>executeCommand("storyboard.new",{path:`${folder}/storyboard.designcraft`,overwrite:false,_expected_project:projectPath}),true)}>Create from the DesignCraft template</button>}
    </main>
    <aside className="sb-panel sb-inspector"><header><h2>Page details &amp; validation</h2><p><StatusMark status={check.status||"incomplete"} issues={0}/> {check.complete?"Storyboard complete":"Storyboard in progress"}</p><p>{check.count??0} unfilled fields · {check.preflight?.errors??0} layout errors</p></header>
      {page?.archetype==="cover"&&<p className="sb-design-order">Start here: map the scenes, assign locations and references, then define the visual direction. Use this plan to develop the world, characters and scene pages.</p>}
      <div className="sb-field-groups">{(page?.groups||[]).map(group=><details key={group} open={group!=="Template formatting"}><summary id={`sb-group-${group.toLowerCase().replace(/[^a-z]+/g,"-")}`}>{group}</summary>{slots.filter(slot=>slot.group===group).map(slot=>{
        const key=slotKey(slot),active=editing===key,state=slotState(slot);
        return <section key={slot.label} className={`sb-field ${active?"editing":""}`} onContextMenu={event=>{event.preventDefault();if(page)copyLink(page.page,slot.label);}}><button id={`sb-edit-${slot.label}`} className="sb-field-heading" aria-expanded={active} aria-describedby={state.issues.length?`sb-issues-${slot.label}`:undefined} title="Click to edit · Right-click to copy a chat link" onClick={()=>setEditing(active?null:key)}><StatusMark status={state.status} issues={state.issues.length}/><span>{slot.display}</span><span>✎</span></button>
          {!!state.issues.length&&<div id={`sb-issues-${slot.label}`}>{state.issues.map((message,index)=><p key={index} className="sb-issue">{message}</p>)}</div>}
          {active&&editor(slot,"sb")}
        </section>;})}</details>)}{report?.issues?.filter(issue=>issue.item==null).map((issue,index)=><p key={index} className="sb-issue">{issue.message}</p>)}</div>
    </aside>
  </div>;
};

export function PreservedStoryboard(props:Props&{active:boolean}){
  const [opened,setOpened]=useState(props.active);
  useEffect(()=>{if(props.active)setOpened(true);},[props.active]);
  return opened||props.active?<div className="sb-preserved" hidden={!props.active}><StoryboardView {...props}/></div>:null;
}
