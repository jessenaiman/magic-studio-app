import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { executeCommand, fileUrl, pickFile, ProjectAsset, StoryboardData } from "../api";
import "../storyboard.css";

interface Slot {id:number;label:string;kind:string;text?:string;image?:string;hidden?:boolean;empty?:boolean}
interface Page {page:number;archetype:string;slots:Slot[]}
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
    <section id="stage-assets-panel" aria-label="Stage Assets & Animate" style={{ gridColumn: "1 / -1", border: "1px solid var(--border, #2a313c)", borderRadius: 8, padding: "8px 12px", marginBottom: 10 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
        <strong>Stage Assets &amp; Animate <span className="muted">({assets.length})</span></strong>
        <button id="stage-tab-characters" className={tab === "characters" ? "start" : "ghost"} onClick={() => setTab("characters")}>Characters / Animate ({characters.length})</button>
        <button id="stage-tab-plates" className={tab === "plates" ? "start" : "ghost"} onClick={() => setTab("plates")}>Backgrounds &amp; Plates ({plates.length})</button>
        <button id="stage-add-asset" className="ghost" style={{ marginLeft: "auto" }} onClick={() => void run(async () => {
          const selected = await pickFile({ title: "Add Asset", extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg", "glb", "gltf"] });
          if (typeof selected === "string" && selected) {
            const name = selected.split("/").pop() || selected;
            await executeCommand("project.add_asset", { path: selected, kind: "image", name, image: name, load: false });
            onToast(`Added ${name}`);
          }
        }, true)}>+ Add Asset</button>
      </div>
      <div style={{ display: "flex", gap: 10, overflowX: "auto", paddingTop: 8 }}>
        {visible.map((asset) => (
          <div
            key={asset.id}
            role="button"
            tabIndex={0}
            title={asset.name || asset.id}
            onClick={() => void run(async () => {
              await executeCommand("character.open", { id: asset.id });
              await executeCommand("ui.set", { view: "animation" });
            })}
            style={{ position: "relative", width: 150, flex: "0 0 auto", border: "1px solid var(--border, #2a313c)", borderRadius: 6, overflow: "hidden", cursor: "pointer" }}
          >
            <button
              id={`stage-asset-remove-${asset.id}`}
              className="ghost"
              aria-label={`Remove ${asset.name || asset.id}`}
              title={`Remove ${asset.name || asset.id} from project`}
              onClick={(event) => {
                event.stopPropagation();
                void run(() => executeCommand("project.remove_asset", { id: asset.id }), true);
              }}
              style={{ position: "absolute", top: 4, right: 4, padding: "0 5px", fontSize: 11, lineHeight: "16px", borderRadius: 8, background: "rgba(0,0,0,0.55)" }}
            >×</button>
            {/\.(png|jpe?g|webp|gif|svg)$/i.test(asset.path || "")
              ? <img src={fileUrl(asset.path)} alt="" style={{ width: "100%", height: 84, objectFit: "cover", display: "block" }} />
              : <div className="muted" style={{ height: 84, display: "flex", alignItems: "center", justifyContent: "center", fontSize: 11 }}>{asset.kind === "model_3d" ? "3D RIG" : "PART"}</div>}
            <div style={{ padding: "6px 8px" }}>
              <div style={{ fontSize: 12, fontWeight: 600, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{asset.name || asset.id}</div>
              <div className="muted" style={{ fontSize: 10, marginTop: 2 }}>{subtitle(asset)}</div>
            </div>
          </div>
        ))}
        {!visible.length && <p className="muted">{tab === "plates" ? "No background plates registered yet." : "No character cutouts registered yet."}</p>}
      </div>
    </section>
  );
};
const colors:Record<string,string>={incomplete:"#ed8796",review:"#eabb62",complete:"#5bd69b"};
const title=(page:Page)=>page.slots.find(slot=>/^(cover_title|world_heading|char_heading|scene_title|scene_id)$/.test(slot.label))?.text||`${page.archetype} ${page.page+1}`;
const friendly=(label:string)=>label.replace(/^(cover|world|char|scene)_/,"").replace(/^songmap_action_(\d+)$/,"Scene $1 · action and location").replace(/^songmap_(\d+)$/,"Scene $1 · visual").replace(/_/g," ");
const isImage=(slot:Slot)=>slot.kind==="image"||slot.image!=null||/_thumb_\d+$/.test(slot.label)||/^cover_songmap_\d+$/.test(slot.label);
const groupOrder=["Scene map","Reference register","Page details","Artwork","Notes and sources","Template formatting"];
const groupLabel=(group:string)=>group==="Reference register"?"Locations & references":group==="Page details"?"Design brief":group;
function groupFor(slot:Slot){
  if(/page_no_|_heading$|_kicker$|_badge$|_revision$|_tag$|_hint$/.test(slot.label))return "Template formatting";
  if(/reg_|ref_|reference/.test(slot.label))return "Reference register";
  if(/scene_map|scene_[1-9]|map_/.test(slot.label))return "Scene map";
  if(isImage(slot))return "Artwork";
  if(/note|caption|credit|source/.test(slot.label))return "Notes and sources";
  return "Page details";
}
let thumbnailQueue:Promise<unknown>=Promise.resolve();
function PageThumbnail({page,project,revision}:{page:Page;project:string;revision:number}){
  const host=useRef<HTMLDivElement>(null),[visible,setVisible]=useState(false),[src,setSrc]=useState("");
  useEffect(()=>{const observer=new IntersectionObserver(rows=>{if(rows.some(row=>row.isIntersecting)){setVisible(true);observer.disconnect();}},{rootMargin:"60px"});if(host.current)observer.observe(host.current);return()=>observer.disconnect();},[]);
  useEffect(()=>{if(!visible)return;let cancelled=false;setSrc("");const out=`${project.replace(/[/\\][^/\\]+$/,"")}/storyboard/.preview/nav-${page.page}.png`;
    const job=thumbnailQueue.catch(()=>{}).then(async()=>{if(cancelled)return;await executeCommand("storyboard.thumbnail",{page:page.page,out,scale:.16,_expected_project:project});if(!cancelled)setSrc(`${fileUrl(out)}?v=${revision}`);});
    thumbnailQueue=job;void job.catch(()=>{});return()=>{cancelled=true;};
  },[visible,page.page,project,revision]);
  return <div className="sb-nav-preview" ref={host}>{src?<img src={src} alt={`Page ${page.page+1}: ${title(page)}`}/>:<span>{page.page+1}</span>}</div>;
}
export const StoryboardView:React.FC<Props>=({projectPath,storyboard,onRefresh,onToast})=>{
  const [pages,setPages]=useState<Page[]>([]),[check,setCheck]=useState<StoryboardData>(storyboard||{}),[pageIndex,setPageIndex]=useState(0);
  const [preview,setPreview]=useState(""),[busy,setBusy]=useState(""),[error,setError]=useState(""),[revision,setRevision]=useState(0),[zoom,setZoom]=useState(100);
  const [drafts,setDrafts]=useState<Record<string,string>>({}),[editing,setEditing]=useState<string|null>(null);
  const request=useRef(0),liveProject=useRef(projectPath);liveProject.current=projectPath;
  const folder=projectPath.replace(/[/\\][^/\\]+$/,"");
  const page=pages[Math.min(pageIndex,Math.max(0,pages.length-1))], report=check.pages?.find(item=>item.page===page?.page);
  const missing=new Set(report?.unfilled.map(item=>item.label)||[]);
  const slotKey=(slot:Slot)=>`${page?.page}:${slot.label}`;
  const slots=(page?.slots||[]).filter(slot=>!slot.hidden&&(slot.kind==="text"||isImage(slot)||slot.text!=null));
  const groups=groupOrder.filter(group=>slots.some(slot=>groupFor(slot)===group));
  const load=async()=>{
    const current=++request.current,project=projectPath;setError("");
    try{
      const data=await executeCommand<{pages:Page[]}>("storyboard.pages",{_expected_project:project});
      const completion=await executeCommand<StoryboardData>("storyboard.check",{_expected_project:project});
      if(current!==request.current||liveProject.current!==project)return;
      setPages(data.pages);setCheck(completion);setRevision(value=>value+1);setPageIndex(index=>Math.min(index,Math.max(0,data.pages.length-1)));
    }catch(cause){if(current===request.current){setError(String(cause));setPreview("");}}
  };
  useEffect(()=>{setPageIndex(0);setDrafts({});setEditing(null);void load();return()=>{++request.current;};},[projectPath]);
  useEffect(()=>{let cancelled=false;setPreview("");if(page){const out=`${folder}/storyboard/.preview/designcraft-${page.page}.png`;
    void executeCommand("storyboard.thumbnail",{page:page.page,out,scale:.7,_expected_project:projectPath}).then(()=>{if(!cancelled)setPreview(`${fileUrl(out)}?v=${revision}`);}).catch(cause=>{if(!cancelled)setError(String(cause));});}return()=>{cancelled=true;};},[page?.page,revision,projectPath]);
  const action=async(name:string,run:()=>Promise<unknown>,refresh=false)=>{if(busy)return;setBusy(name);setError("");try{await run();if(refresh){await load();await onRefresh();}}catch(cause){setError(String(cause));onToast(String(cause));}finally{setBusy("");}};
  const change=(slot:Slot,value:string)=>{const key=slotKey(slot);setDrafts(old=>{const next={...old};if(value===(slot.text??slot.image??""))delete next[key];else next[key]=value;return next;});};
  const save=async(slot:Slot)=>{const key=slotKey(slot),value=drafts[key];if(value===undefined||!page)return;
    await action("Saving field…",async()=>{await executeCommand("storyboard.set",{page:page.page,[isImage(slot)?"images":"fields"]:{[slot.label]:value},_expected_project:projectPath});setDrafts(old=>{const next={...old};delete next[key];return next;});setEditing(null);},true);
  };
  return <div id="view-storyboard" className="storyboard-layout" data-document-dirty={Object.keys(drafts).length>0} data-document-saving={busy==="Saving field…"} data-document-label="Storyboard fields">
    <StageAssets onToast={onToast} />
    <aside className="sb-panel sb-index"><header><h2>Storyboard &amp; design index</h2><p>DesignCraft · {pages.length} pages</p></header>
      <button id="storyboard-refresh" className="ghost" disabled={!!busy} onClick={()=>void action("Refreshing…",async()=>{await load();await onRefresh();})}>Sync &amp; refresh</button>
      <nav aria-label="Storyboard pages">{pages.map((item,index)=>{const status=check.pages?.find(p=>p.page===item.page)?.status||"incomplete";return <button id={`sb-nav-${item.page+1}`} key={item.page} className={`sb-page-card ${page?.page===item.page?"selected":""}`} aria-current={page?.page===item.page?"page":undefined} onClick={()=>{setPageIndex(index);setEditing(null);}}>
        <PageThumbnail page={item} project={projectPath} revision={revision}/><span><i style={{color:colors[status]}} aria-label={status}>●</i> {item.page+1}. {title(item)}</span></button>;})}</nav>
      <button id="storyboard-export" className="ghost" disabled={!!busy||!pages.length} onClick={()=>void action("Exporting PDF…",async()=>{await executeCommand("storyboard.export",{pdf:`${folder}/storyboard.pdf`,_expected_project:projectPath});onToast("Saved storyboard.pdf");})}>Export PDF</button>
    </aside>
    <main className="sb-panel sb-main"><header className="sb-toolbar"><button id="sb-prev" className="ghost" disabled={pageIndex===0} onClick={()=>{setPageIndex(i=>i-1);setEditing(null);}}>←</button><strong>{page?title(page):"DesignCraft storyboard"}</strong><button id="sb-next" className="ghost" disabled={!pages.length||pageIndex>=pages.length-1} onClick={()=>{setPageIndex(i=>i+1);setEditing(null);}}>→</button>
      <label>Zoom <select id="sb-zoom" value={zoom} onChange={event=>setZoom(Number(event.target.value))}><option value={100}>Fit</option><option value={150}>150%</option><option value={200}>200%</option></select></label>
      <button id="storyboard-open-editor" className="ghost" disabled={!!busy||!pages.length} onClick={()=>void action("Opening DesignCraft…",()=>executeCommand("storyboard.open",{_expected_project:projectPath}))}>Open in DesignCraft ↗</button></header>
      <div id="sb-cards" className={`sb-spread ${zoom===100?"fit":"zoomed"}`}>{preview?<img src={preview} alt={page?title(page):"Storyboard page"} style={zoom===100?undefined:{width:`${zoom}%`,maxWidth:"none"}}/>:<p>{pages.length?"Rendering saved DesignCraft page…":"No readable DesignCraft storyboard yet."}</p>}</div>
      {error&&<p role="alert" className="sb-error">{error}</p>}<footer role="status">{busy||`${Object.keys(drafts).length?`${Object.keys(drafts).length} unsaved field(s) · `:""}Edit a field on the right, or open the native document. Navigation stays unrestricted.`}</footer>
      {!pages.length&&<button id="storyboard-new" className="start" disabled={!!busy} onClick={()=>void action("Creating storyboard…",()=>executeCommand("storyboard.new",{path:`${folder}/storyboard.designcraft`,overwrite:false,_expected_project:projectPath}),true)}>Create from the DesignCraft template</button>}
    </main>
    <aside className="sb-panel sb-inspector"><header><h2>Page details &amp; validation</h2><p><span style={{color:colors[check.status||"incomplete"]}}>●</span> {check.complete?"Storyboard complete":"Storyboard in progress"}</p><p>{check.count??0} unfilled fields · {check.preflight?.errors??0} layout errors</p></header>
      {page?.archetype==="cover"&&<p className="sb-design-order">Start here: map the scenes, assign locations and references, then define the visual direction. Use this plan to develop the world, characters and scene pages.</p>}
      <div className="sb-field-groups">{groups.map(group=><details key={group} open={group!=="Template formatting"}><summary id={`sb-group-${group.toLowerCase().replace(/\s/g,"-")}`}>{groupLabel(group)}</summary>{slots.filter(slot=>groupFor(slot)===group).map(slot=>{
        const key=slotKey(slot),image=isImage(slot),value=drafts[key]??slot.text??slot.image??"",active=editing===key;
        const issues=(check.preflight?.issues||[]).filter(issue=>issue.item===slot.id);
        const status=missing.has(slot.label)||issues.some(issue=>issue.severity==="error")?"incomplete":issues.length?"review":"complete";
        const empty=image&&(slot.empty===true||(slot.empty===undefined&&!slot.image));
        const optionalEmpty=empty&&!missing.has(slot.label)&&!issues.length;
        return <section key={slot.label} className={`sb-field ${active?"editing":""}`}><button id={`sb-edit-${slot.label}`} className="sb-field-heading" aria-expanded={active} aria-describedby={issues.length?`sb-issues-${slot.label}`:undefined} onClick={()=>setEditing(active?null:key)}><span style={{color:optionalEmpty?"var(--muted)":colors[status]}} aria-label={optionalEmpty?"Empty visual; no required-field finding":status}>{issues.length?"⚠":missing.has(slot.label)?"●":optionalEmpty?"○":"✓"}</span><span>{friendly(slot.label)}</span><span>✎</span></button>
          {!!issues.length&&<div id={`sb-issues-${slot.label}`}>{issues.map((issue,index)=><p key={index} className="sb-issue">{issue.message}</p>)}</div>}
          {active&&<div className="sb-field-editor">{image?<><input id={`sb-field-${slot.label}`} aria-label={friendly(slot.label)} value={value} onChange={event=>change(slot,event.target.value)}/><button id={`sb-choose-${slot.label}`} className="ghost" onClick={()=>void pickFile({title:`Choose ${friendly(slot.label)}`,extensions:["png","jpg","jpeg","webp","svg"]}).then(path=>{if(typeof path==="string"&&liveProject.current===projectPath)change(slot,path);}).catch(cause=>setError(String(cause)))}>Choose image…</button></>:<textarea id={`sb-field-${slot.label}`} aria-label={friendly(slot.label)} rows={4} value={value} onChange={event=>change(slot,event.target.value)}/>}
            <div><button id={`sb-save-${slot.label}`} className="start" disabled={!!busy||drafts[key]===undefined} onClick={()=>void save(slot)}>Save field</button><button id={`sb-cancel-${slot.label}`} className="ghost" disabled={!!busy} onClick={()=>{setDrafts(old=>{const next={...old};delete next[key];return next;});setEditing(null);}}>Cancel</button></div></div>}
        </section>;})}</details>)}{report?.issues?.map((issue,index)=><p key={index} className="sb-issue">{issue.message}</p>)}</div>
      <details className="sb-cli"><summary id="sb-cli-toggle">CLI and live control</summary><code>magic-cli --bridge 127.0.0.1:9876 exec storyboard.set --params '{JSON.stringify({page:page?.page||0,fields:{cover_title:"Project title"}})}'</code></details>
    </aside>
  </div>;
};

export function PreservedStoryboard(props:Props&{active:boolean}){
  const [opened,setOpened]=useState(props.active);
  useEffect(()=>{if(props.active)setOpened(true);},[props.active]);
  return opened||props.active?<div style={{display:props.active?"block":"none",height:"100%"}}><StoryboardView {...props}/></div>:null;
}
