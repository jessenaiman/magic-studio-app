// Connect the preserved pre-egui inspectors to the current GLB and Theatre data.
function connectRestoredAnimation() {
  const show3D = animationView.show3D.bind(animationView);
  animationView.show3D = async (...args) => {
    await show3D(...args);
    if (!motion.viewport3d || !tripo3DStage.modelRoot || !motion.meta) return;
    const bones = [];
    tripo3DStage.modelRoot.traverse(node => { if (node.isBone) bones.push(node); });
    motion.meta.layers = bones.map((bone, order) => ({ id: bone.name, label: bone.name.replace(/^mixamorig[:_]?/, ""), parent: bone.parent?.name, order }));
    motion.meta.tracks = tripo3DStage.animations.map(clip => ({ id: clip.name, label: clip.name.split("|").pop(), duration: clip.duration, loop: true, standard: false }));
    motion.selected = bones[0]?.name || null;
    const clip = motion.meta.tracks.find(track => track.id === motion.track) || motion.meta.tracks[0];
    if (clip) { motion.track = clip.id; motion.rest = false; motion.sample = { duration: clip.duration, loop: true, fps: 30, frames: [] }; }
  };
  const sample = animationView.sample.bind(animationView);
  animationView.sample = async () => {
    if (!motion.viewport3d) return sample();
    if (!motion.rest) {
      const track = motion.meta?.tracks.find(track => track.id === motion.track);
      if (track) {
        tripo3DStage.playClip([track.id], 0);
        motion.sample = { start: 0, duration: track.duration, loop: motion.previewLoop ?? track.loop, fps: 30, frames: [] };
        $("motion-scrub").max = track.duration;
      }
    }
    animationDraw();
  };
  const compare = animationView.buildPartComparePanel?.bind(animationView);
  if (compare) animationView.buildPartComparePanel = (...args) => motion.viewport3d
    ? el("p", { class: "muted" }, "This is a skeleton bone. Edit the character's artwork in its native editor.") : compare(...args);
  const controls = animationView.renderControls.bind(animationView);
  animationView.renderControls = () => {
    controls();
    if (!motion.viewport3d) return;
    const pen = $("motion-pen");
    if (pen) { pen.disabled = true; pen.title = "Draw paths on a 2D character canvas"; }
    const action = $("motion-action") || document.querySelector('#motion-inspector .motion-section-body');
    const track = animationView.buildTrackSelector(motion.meta);
    (action?.querySelector?.('.motion-section-body') || action)?.prepend(track);
    const layerList = document.querySelector("#motion-layers .motion-layers");
    if (layerList) { layerList.style.maxHeight = "190px"; layerList.style.overflowY = "auto"; }
    const part = motion.selected;
    const field = new InspectorNumberField({ id: "motion-part-angle", label: "Rotation", min: -45, max: 45, step: 1, value: magicTheatre.rotations[part] || 0, unit: "°", control: "dial", change: value => {
      if (!part) return;
      magicTheatre.rotations[part] = value;
      magicTheatre.frame?.contentWindow.postMessage({ magicTheatre: true, type: "set_rotation", part, rotation: value }, location.origin);
      animationDraw();
    } });
    const expression = motionSection("Expression & pose", [el("p", { class: "muted" }, part?.replace(/^mixamorig[:_]?/, "") || "Select a bone"), field.node], true);
    $("motion-layers")?.after(expression);
    void theatreContext();
  };
  const seek = tripo3DStage.seek.bind(tripo3DStage);
  const base = new Map();
  tripo3DStage.seek = time => {
    for (const [bone, rotation] of base) bone.quaternion.copy(rotation);
    base.clear();
    if (tripo3DStage.activeAction) tripo3DStage.activeAction.setEffectiveWeight(motion.rest ? 0 : motion.amount / 100);
    seek(time);
    for (const [name, degrees] of Object.entries(magicTheatre.rotations)) {
      const bone = tripo3DStage.modelRoot?.getObjectByName(name);
      if (bone) { base.set(bone, bone.quaternion.clone()); bone.rotateZ(THREE.MathUtils.degToRad(degrees)); }
    }
  };
  const match = modelClipMatch;
  modelClipMatch = (available, candidates) => match(available, candidates) || available.find(name => {
    const words = name.toLowerCase().split(/[^a-z0-9]+/);
    return candidates.some(candidate => words.includes(candidate.toLowerCase()));
  }) || null;
  magicTheatre.saveEmptyExport = async () => { $("theatre-status").textContent = "No timeline edits to save"; return { saved: false }; };
  const exportDrawing = characterExportClip;
  characterExportClip = async status => {
    if (!motion.viewport3d) return exportDrawing(status);
    if (Object.values(magicTheatre.rotations).some(value => Math.abs(value) > .0001))
      throw new Error("Timeline pose edits are saved in the project. Use Drawing / motion review to bake corrections into a revised GLB before rendering it.");
    const clip = tripo3DStage.activeAction?.getClip().name;
    if (!clip) throw new Error("Choose a model clip first");
    status.textContent = "Preparing this model clip in EffectCraft…";
    const prepared = await motionCommand("animation.prepare_motion", { character: motion.character, clip, option: "baseline", duration: motion.sample?.duration || tripo3DStage.duration });
    status.textContent = "Rendering the prepared EffectCraft clip…";
    const result = await motionCommand("character.render", { character: motion.character, effectcraft_project: prepared.project, start: 0, end: motion.sample?.duration || tripo3DStage.duration, fps: 30, format: "clip" });
    if (!result.clip) throw new Error("The compositor returned no rendered clip");
    motion.lastExport = { project: state.project, character: motion.character, action: clip, result };
    status.textContent = "Clip rendered. Place it in the film after reviewing it.";
    status.append(fileButton({ path: result.clip, name: result.clip.split("/").pop() }));
  };

}
