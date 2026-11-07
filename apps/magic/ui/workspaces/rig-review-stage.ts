import * as T from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';

type Correction = {bone:string;time:number;rotation:number[]};
class RigReviewStage {
  renderer:T.WebGLRenderer;
  actors:any[]=[];
  focus='all'; zoom=1; skeleton=false; wireframe=false; grid=true;
  clip='idle'; time=0; drafts:Record<string,any>={};
  observer:ResizeObserver;
  disposed=false;
  constructor(public host:HTMLElement) {
    this.renderer=new T.WebGLRenderer({antialias:true,preserveDrawingBuffer:true});
    this.renderer.setPixelRatio(Math.min(devicePixelRatio,2));
    this.renderer.toneMapping=T.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure=1.15;
    host.append(this.renderer.domElement);
    this.observer=new ResizeObserver(()=>this.render());this.observer.observe(host);
  }
  async load(items:any[]) {
    for(const item of items) {
      const gltf=await new GLTFLoader().loadAsync(item.url);
      if(this.disposed){this.release(gltf.scene);return;}
      const model=gltf.scene,scene=new T.Scene();scene.background=new T.Color('#20332a');
      scene.add(new T.HemisphereLight(0xf8ffe8,0x57634f,2.6));
      for(const [color,intensity,pos] of [[0xffedcf,3.3,[3,6,4]],[0xd5eaff,2,[-4,3,1]],[0xd8f4a3,2.5,[1,4,-3]]] as any[]) {
        const light=new T.DirectionalLight(color,intensity);light.position.set(...pos as [number,number,number]);scene.add(light);
      }
      const wrap=new T.Group(),orient=new T.Group();orient.add(model);wrap.add(orient);scene.add(wrap);
      model.updateMatrixWorld(true);const box=new T.Box3().setFromObject(model,true),height=box.getSize(new T.Vector3()).y||1;
      wrap.scale.setScalar(2.5/height);wrap.updateMatrixWorld(true);
      box.setFromObject(model,true);const center=box.getCenter(new T.Vector3());wrap.position.set(-center.x,-box.min.y,-center.z);
      const bones:any[]=[];model.traverse((node:any)=>{if(node.isBone)bones.push(node);if(node.isSkinnedMesh)node.frustumCulled=false;});
      const left=bones.find(b=>/LeftArm$/.test(b.name)),right=bones.find(b=>/RightArm$/.test(b.name));
      if(left&&right){const f=left.getWorldPosition(new T.Vector3()).sub(right.getWorldPosition(new T.Vector3())).cross(new T.Vector3(0,1,0)).normalize();orient.rotation.y=-Math.atan2(f.x,f.z);}
      const floor=new T.GridHelper(10,40,0x647558,0x435647);scene.add(floor);
      const helper=new T.SkeletonHelper(model);for(const material of (Array.isArray(helper.material)?helper.material:[helper.material]))material.depthTest=false;helper.renderOrder=10;scene.add(helper);
      const front=new T.PerspectiveCamera(34,1,.01,200),side=front.clone();
      this.actors.push({item,model,scene,wrap,orient,origin:wrap.position.clone(),bones,mixer:new T.AnimationMixer(model),clips:gltf.animations,front,side,helper,floor,action:null,base:new Map(),bounds:null});
    }
    this.select(this.clip);
  }
  select(name:string) {
    this.clip=name;
    for(const a of this.actors){
      this.restore(a);a.mixer.stopAllAction();a.bones.forEach((b:any)=>{b.position.copy(b.userData.restPosition||b.position);b.quaternion.copy(b.userData.restQuaternion||b.quaternion);});
      if(!a.initialized){a.bones.forEach((b:any)=>{b.userData.restPosition=b.position.clone();b.userData.restQuaternion=b.quaternion.clone();});a.initialized=true;}
      const clip=a.clips.find((c:any)=>c.name===name);
      a.action=clip?a.mixer.clipAction(clip).reset().setLoop(T.LoopOnce,1):null;
      if(a.action){a.action.clampWhenFinished=true;a.action.play();}
      // Frame the entire motion, including lying poses, without recentering its height.
      const bounds=new T.Box3();
      for(let i=0;i<=12;i++){this.pose(a,clip?clip.duration*i/12:0);bounds.union(new T.Box3().setFromObject(a.model,true));}
      a.bounds=bounds;
    }
    this.render();
  }
  restore(a:any) {for(const [bone,q] of a.base)bone.quaternion.copy(q);a.base.clear();}
  correction(keys:Correction[],bone:string,time:number) {
    const frames=keys.filter(k=>k.bone===bone).sort((a,b)=>a.time-b.time);
    if(!frames.length)return null;
    // Explicit keys hold at the boundaries; add a zero-angle key to bound a correction.
    let l=frames[0],r=frames[frames.length-1];
    for(const k of frames){if(k.time<=time)l=k;if(k.time>=time){r=k;break;}}
    const q=(k:Correction)=>new T.Quaternion().setFromEuler(new T.Euler(...k.rotation.map(T.MathUtils.degToRad) as [number,number,number]));
    return q(l).slerp(q(r),r.time===l.time?0:T.MathUtils.clamp((time-l.time)/(r.time-l.time),0,1));
  }
  pose(a:any,time:number) {
    this.restore(a);
    if(a.action){a.action.enabled=true;a.action.paused=false;a.action.time=Math.min(time,a.action.getClip().duration);a.mixer.update(0);}
    const draft=this.drafts[a.item.id];
    if(draft)for(const b of a.bones){const q=this.correction(draft.keys||[],b.name,time);if(q){a.base.set(b,b.quaternion.clone());b.quaternion.multiply(q);}}
    a.wrap.position.copy(a.origin);a.wrap.updateMatrixWorld(true);
    const hip=a.bones.find((b:any)=>/Hips$/.test(b.name));
    if(hip&&draft?.in_place!==false){const p=hip.getWorldPosition(new T.Vector3());a.wrap.position.x-=p.x;a.wrap.position.z-=p.z;}
    a.wrap.updateMatrixWorld(true);
  }
  render() {
    if(this.disposed||!this.host.clientWidth||!this.host.clientHeight)return;
    const width=this.host.clientWidth,height=this.host.clientHeight;
    this.renderer.setSize(width,height,false);this.renderer.setScissorTest(true);
    const active=this.actors.filter(a=>this.focus==='all'||a.item.id===this.focus);
    const cols=active.length===1?1:Math.min(2,active.length),rows=Math.ceil(active.length/cols)||1;
    for(let i=0;i<active.length;i++){
      const a=active[i];this.pose(a,this.time);a.helper.visible=this.skeleton;a.floor.visible=this.grid;
      a.model.traverse((n:any)=>{if(n.isMesh)for(const m of Array.isArray(n.material)?n.material:[n.material])if(m)m.wireframe=this.wireframe;});
      const bounds=a.bounds||new T.Box3().setFromObject(a.model,true),center=bounds.getCenter(new T.Vector3()),size=bounds.getSize(new T.Vector3());
      const w=width/cols,h=height/rows,y=height-(Math.floor(i/cols)+1)*h;
      for(let j=0;j<2;j++){
        const c=j?a.side:a.front;c.aspect=w/2/h;
        const half=Math.max(size.y/2,size.x/2/c.aspect,size.z/2/c.aspect,1.25)/this.zoom;
        const distance=half/Math.tan(T.MathUtils.degToRad(c.fov/2))*1.12;
        c.position.copy(center).add(new T.Vector3(j?distance:0,.08,distance*(j?0:1)));c.lookAt(center);c.updateProjectionMatrix();
        this.renderer.setViewport((i%cols)*w+j*w/2,y,w/2,h);this.renderer.setScissor((i%cols)*w+j*w/2,y,w/2,h);this.renderer.render(a.scene,c);
      }
    }
  }
  seek(time:number,drafts:Record<string,any>) {this.time=time;this.drafts=drafts;this.render();}
  bones(id:string):string[]{return this.actors.find(a=>a.item.id===id)?.bones.map((b:any)=>b.name)||[];}
  duration(id?:string):number{return Math.max(0,...this.actors.filter(a=>!id||a.item.id===id).map(a=>a.clips.find((c:any)=>c.name===this.clip)?.duration||0));}
  async exportClip(id:string,draft:any):Promise<ArrayBuffer> {
    const a=this.actors.find(a=>a.item.id===id),source=a?.clips.find((c:any)=>c.name===this.clip);
    if(!source)throw Error('This character has no selected animation');
    const start=draft.trim_start,end=Math.min(source.duration,draft.trim_end),duration=(end-start)/draft.speed;
    const times:number[]=[], values=new Map<any,{position:number[];quaternion:number[];scale:number[]}>();
    for(const bone of a.bones)values.set(bone,{position:[],quaternion:[],scale:[]});
    for(let i=0,n=Math.ceil(duration*30);i<=n;i++){
      const t=Math.min(i/30,duration);times.push(t);this.pose(a,start+t*draft.speed);
      for(const [bone,v] of values){v.position.push(...bone.position.toArray());v.quaternion.push(...bone.quaternion.toArray());v.scale.push(...bone.scale.toArray());}
    }
    const tracks:T.KeyframeTrack[]=[];
    for(const [bone,v] of values){tracks.push(new T.VectorKeyframeTrack(`${bone.name}.position`,times,v.position),new T.QuaternionKeyframeTrack(`${bone.name}.quaternion`,times,v.quaternion),new T.VectorKeyframeTrack(`${bone.name}.scale`,times,v.scale));}
    const clip=new T.AnimationClip(source.name,duration,tracks);
    this.restore(a);a.mixer.stopAllAction();a.bones.forEach((b:any)=>{b.position.copy(b.userData.restPosition);b.quaternion.copy(b.userData.restQuaternion);});
    // Export the source model, excluding inspection-only normalization and overlays.
    let output:ArrayBuffer;
    try{output=await new GLTFExporter().parseAsync(a.model,{binary:true,animations:[clip],onlyVisible:false}) as ArrayBuffer;}
    finally{a.action.reset().play();this.render();}
    return output;
  }
  release(root:any){root.traverse((n:any)=>{n.geometry?.dispose();for(const m of (Array.isArray(n.material)?n.material:[n.material]))if(m){for(const v of Object.values(m))if((v as any)?.isTexture)(v as any).dispose();m.dispose();}});}
  dispose(){this.disposed=true;this.observer.disconnect();for(const a of this.actors){a.mixer.stopAllAction();this.release(a.model);a.helper.dispose();a.floor.geometry.dispose();a.floor.material.dispose();}this.renderer.dispose();this.host.replaceChildren();}
}
(window as any).RigReviewStage=RigReviewStage;

// The existing Animation viewport and Scribbles share the same Three.js runtime.
(window as any).THREE={...T,GLTFLoader,OrbitControls};
