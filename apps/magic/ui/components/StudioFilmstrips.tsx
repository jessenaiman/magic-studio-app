import { useEffect, useRef, useState } from "react";
import { executeCommand, fileUrl, TimelineClip, TimelineResponse } from "../api";
import "../studio-filmstrips.css";

type Clip = TimelineClip & {clip?:number;item?:number;sourceIn?:number;source_path?:string};
const clock = (time:number) => `${Math.floor(time/60)}:${(time%60).toFixed(1).padStart(4,"0")}`;
const cache = new Map<string,Promise<string>>();
let queue:Promise<unknown> = Promise.resolve();
function frameImage(project:string,time:number,revision:number,clip?:Clip,kind?:"lyrics"|"motion"):Promise<string> {
  const at = Math.max(0,time), key = `${project}:${revision}:${kind || clip?.item || "film"}:${at.toFixed(2)}`;
  const existing = cache.get(key); if (existing) return existing;
  const ready = clip?.item ? frameImage(project,0,revision) : Promise.resolve("");
  const result = ready.then(base => {
    const job = queue.catch(()=>{}).then(async () => {
      if (kind) {
        const value=await executeCommand<{frame?:string;empty?:boolean}>("words.preview_frame",{time:at,kind,_expected_project:project});
        return value.frame || "";
      }
      if (clip?.item) {
        const path = base.replace(/[^/]+$/,`source-${clip.item}-${Math.round(at*1000)}.png`);
        const time = Math.round(((clip.sourceIn || 0) + Math.max(0,at-clip.start))*254016000000);
        await executeCommand("video.exec",{id:"file.exportFrame",params:{target:"source",item:clip.item,time,path,format:"png",import:false},save:false,_expected_project:project});
        return path;
      }
      return (await executeCommand<{frame:string}>("video.frame",{seconds:at,_expected_project:project})).frame;
    });
    queue=job; return job;
  });
  cache.set(key,result);
  // Authoring documents can change without a FilmCraft timeline revision. Only
  // share in-flight native authoring renders; a later visit must read the file again.
  if(kind) void result.then(()=>cache.delete(key),()=>cache.delete(key));
  result.catch(()=>cache.delete(key));
  return result;
}
function NativeFrame({project,time,revision,clip,kind}:{project:string;time:number;revision:number;clip?:Clip;kind?:"lyrics"|"motion"}) {
  const host=useRef<HTMLDivElement>(null);
  const [visible,setVisible]=useState(false),[path,setPath]=useState(""),[error,setError]=useState(""),[loaded,setLoaded]=useState(false);
  useEffect(()=>{
    const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){setVisible(true);observer.disconnect();}},{rootMargin:"80px"});
    if(host.current) observer.observe(host.current); return()=>observer.disconnect();
  },[]);
  useEffect(()=>{
    if(!visible)return;let cancelled=false;setPath("");setError("");setLoaded(false);
    frameImage(project,time,revision,clip,kind).then(path=>{if(!cancelled){setPath(path);setLoaded(true);}}).catch(cause=>{if(!cancelled)setError(String(cause));});
    return()=>{cancelled=true;};
  },[visible,project,time,revision,clip?.item,kind]);
  return <div ref={host} className="studio-thumb" title={error || undefined}>{path?<img src={`${fileUrl(path)}?v=${revision}`} alt={clip?.name || `Film at ${clock(time)}`}/>:<span>{error?"Native frame unavailable":loaded?"":"Preparing frame…"}</span>}</div>;
}
export function StudioFilmstrips({project,timeline,videoSrc,revision,playhead,onTime,videoRef}:{project:string;timeline:TimelineResponse|null;videoSrc:string;revision:number;playhead:number;onTime:(time:number)=>void;videoRef:React.RefObject<HTMLVideoElement|null>}){
  const media=videoSrc;
  const [mediaError,setMediaError]=useState(""),[mediaReady,setMediaReady]=useState(false);
  const [playing,setPlaying]=useState(false),[volume,setVolume]=useState(1),[poster,setPoster]=useState("");
  useEffect(()=>{if(playing)return;let stop=false;frameImage(project,playhead,revision).then(path=>{if(!stop)setPoster(fileUrl(path));}).catch(()=>{});return()=>{stop=true;};},[project,revision,playhead,playing]);
  useEffect(()=>{setMediaError("");setMediaReady(false);setPlaying(false);},[videoSrc]);
  const tracks=timeline?.tracks||[], duration=timeline?.duration||1;
  const backgrounds=(tracks.find(track=>track.label==="V1")?.clips||[]) as Clip[];
  const characters=tracks.filter(track=>track.kind==="video"&&track.label!=="V1").flatMap(track=>track.clips).filter(clip=>/scene-|\/characters\//.test(clip.name+((clip as Clip).source_path||""))) as Clip[];
  const scenes=backgrounds.length?backgrounds:tracks.filter(track=>track.kind==="video").flatMap(track=>track.clips).slice(0,16) as Clip[];
  const selected=scenes.findIndex(clip=>playhead>=clip.start&&playhead<clip.end);
  const scene=scenes[Math.max(0,selected)];
  const rangeStart=Math.max(0,(scene?.start||0)-2),rangeEnd=Math.min(duration,Math.max(rangeStart+8,scene?.end||8)+2);
  const samples=Array.from({length:5},(_,i)=>rangeStart+(rangeEnd-rangeStart)*i/5);
  const nearest=samples.reduce((best,t,i)=>Math.abs(t-playhead)<Math.abs(samples[best]-playhead)?i:best,0);
  const characterAt=(time:number)=>characters.find(clip=>time>=clip.start&&time<clip.end);
  const seek=(time:number)=>{onTime(time);if(videoRef.current)videoRef.current.currentTime=time;};
  const edit=(view:string)=>void executeCommand("ui.set",{view,...(["design","motion"].includes(view)?{time:playhead}:{})}).catch(cause=>setMediaError(String(cause)));
  const element=(kind:"animation"|"lyrics"|"motion",time:number)=>kind==="animation"?(characterAt(time)?<NativeFrame project={project} time={time} revision={revision} clip={characterAt(time)}/>:<div className="studio-thumb" aria-label="No animation at this moment"/>):<NativeFrame project={project} time={time} revision={revision} kind={kind}/>;
  return <div className="studio-filmstrip-layout">
    <aside className="studio-panel studio-scenes"><h2>Film / Scenes</h2><div className="studio-scenes-scroll">{scenes.map((clip,index)=><button key={clip.clip??index} id={`studio-scene-${index}`} className={`studio-scene ${index===selected?"selected":""}`} onClick={()=>seek(clip.start)}>
      <div className="studio-perforated"><NativeFrame project={project} time={Math.min(clip.end-.01,clip.start+.15)} revision={revision}/><time>{clock(clip.start)}</time></div><span>Scene {index+1} · {clip.name.replace(/\.[^.]+$/,"").replace(/-/g," ")}</span></button>)}</div></aside>
    <main className="studio-review-center"><section className="studio-panel studio-program"><div className="studio-monitor"><video id="film-player" ref={videoRef} src={media||undefined} poster={poster||undefined} preload="metadata" onPlay={()=>setPlaying(true)} onPause={()=>setPlaying(false)} onLoadedData={()=>setMediaReady(true)} onError={()=>{setMediaReady(false);setPlaying(false);setMediaError("Video playback unavailable. Open in FilmCraft to review the timeline.");}} onLoadedMetadata={()=>{if(videoRef.current)videoRef.current.currentTime=playhead;}} onTimeUpdate={event=>onTime(event.currentTarget.currentTime)}/>{!media&&<NativeFrame project={project} time={playhead} revision={revision}/>}</div><div className="studio-transport"><button id="film-previous" className="ghost" aria-label="Previous scene" onClick={()=>seek(scenes[Math.max(0,selected-1)]?.start||0)}>◀</button><button id="film-play" className="start" disabled={!media||!mediaReady} aria-label={playing?"Pause video":"Play video"} onClick={()=>{const video=videoRef.current;if(!video)return;if(playing)video.pause();else void video.play().catch(cause=>setMediaError(String(cause)));}}>{playing?"Ⅱ":"▶"}</button><button id="film-next" className="ghost" aria-label="Next scene" onClick={()=>seek(scenes[Math.min(scenes.length-1,selected+1)]?.start||0)}>▶</button><time>{clock(playhead)}</time><input id="film-seek" aria-label="Film time" type="range" min={0} max={duration} step={.01} value={playhead} onChange={event=>seek(Number(event.target.value))}/><input id="film-volume" aria-label="Volume" type="range" min={0} max={1} step={.05} value={volume} onChange={event=>{const value=Number(event.target.value);setVolume(value);if(videoRef.current)videoRef.current.volume=value;}}/><button id="film-fullscreen" className="ghost" aria-label="Fullscreen preview" onClick={()=>void videoRef.current?.requestFullscreen().catch(cause=>setMediaError(String(cause)))}>⛶</button></div>{mediaError&&<p role="alert">{mediaError}</p>}</section>
      <section className="studio-panel studio-moment"><h2>At this moment <time>{clock(playhead)}</time></h2><div className="studio-moment-grid">{(["animation","lyrics","motion"] as const).map(kind=><article key={kind}>{element(kind,Math.round(playhead*2)/2)}<footer><span className={`studio-kind ${kind}`}>{kind[0].toUpperCase()+kind.slice(1)}</span><button id={`studio-edit-${kind}`} className="ghost" onClick={()=>edit(kind==="lyrics"?"design":kind)}>Edit in…</button></footer></article>)}</div></section>
    </main>
    <aside className="studio-panel studio-elements"><h2>Elements</h2><div className="studio-element-head"><span/>{["Animation","Lyrics","Motion"].map(label=><strong key={label} className={`studio-kind ${label.toLowerCase()}`}>{label}</strong>)}</div><div className="studio-element-scroll"><div className="studio-element-frames">{samples.map((time,index)=><div key={time} className={`studio-element-row ${index===nearest?"current":""}`}><button id={`studio-time-${index}`} className="studio-time" onClick={()=>seek(time)}>{clock(time)}</button>{(["animation","lyrics","motion"] as const).map(kind=><button id={`studio-element-${kind}-${index}`} key={kind} className={`studio-perforated ${kind}`} aria-label={`${kind} at ${clock(time)}`} onClick={()=>seek(time)}>{element(kind,time)}</button>)}</div>)}<div className="studio-element-playhead" style={{top:`${Math.max(0,Math.min(100,((playhead-rangeStart)/(rangeEnd-rangeStart)+.1)*100))}%`}}><time>{clock(playhead)}</time></div></div></div></aside>
    <section id="film-layers" className="studio-panel studio-range"><h2>Review range <span>{clock(rangeStart)}–{clock(rangeEnd)}</span></h2><div className="studio-range-ruler"><span/>{samples.map(time=><span key={time}>{clock(time)}</span>)}</div>{(["film","animation","lyrics","motion"] as const).map(kind=><div key={kind} className={`studio-range-row ${kind}`}><strong className={`studio-kind ${kind}`}>{kind[0].toUpperCase()+kind.slice(1)}</strong><div className="studio-range-strip">{samples.map((time,index)=><button id={`studio-range-${kind}-${index}`} key={time} aria-label={`Review ${kind} at ${clock(time)}`} onClick={()=>seek(time)}>{kind==="film"?<NativeFrame project={project} time={time} revision={revision}/>:element(kind,time)}</button>)}{scene&&<div className="studio-active-range" title="Current scene" style={{left:`${Math.max(0,(scene.start-rangeStart)/(rangeEnd-rangeStart)*100)}%`,width:`${Math.min(100,(scene.end-scene.start)/(rangeEnd-rangeStart)*100)}%`}}/>}<div className="studio-playhead" style={{left:`${Math.max(0,Math.min(100,(playhead-rangeStart)/(rangeEnd-rangeStart)*100))}%`}}/></div></div>)}</section>
  </div>;
}
