import React, { useEffect, useState } from "react";
import {
  CharacterMetaResponse,
  CharacterPart,
  CharacterSpriteSummary,
  CharacterSummary,
  executeCommand,
  ProjectAsset,
} from "../api";
import { AnimationPoseInspector } from "./animation/AnimationPoseInspector";
import { AnimationStage, BoneNode, SpriteAction } from "./animation/AnimationStage";
import { AnimationTheatre } from "./animation/AnimationTheatre";

interface AnimationViewProps {
  projectPath: string;
  initialCharacterId?: string;
  assets: ProjectAsset[];
  characters: CharacterSummary[];
  onSelectCharacter?: (charId: string) => void;
  onRefresh: () => Promise<void>;
  onToast: (msg: string) => void;
}

const DEFAULT_BONES: BoneNode[] = [
  { id: "head", label: "head", x: 192, y: 165, parent: "torso" },
  { id: "arm_R", label: "arm_R", x: 120, y: 265, parent: "torso" },
  { id: "arm_L", label: "arm_L", x: 264, y: 265, parent: "torso" },
  { id: "torso", label: "torso", x: 192, y: 280, parent: null },
  { id: "leg_R", label: "leg_R", x: 160, y: 365, parent: "torso" },
  { id: "leg_L", label: "leg_L", x: 224, y: 365, parent: "torso" },
];

export const AnimationView: React.FC<AnimationViewProps> = ({
  projectPath,
  initialCharacterId,
  characters,
  onSelectCharacter,
  onToast,
}) => {
  const [selectedCharId, setSelectedCharId] = useState<string>(initialCharacterId || "");
  const [selectedView, setSelectedView] = useState<string>("front");

  // Stage overlays: showPath on by default so spline path & timing HUD are immediately accessible
  const [showMarks, setShowMarks] = useState<boolean>(false);
  const [showPath, setShowPath] = useState<boolean>(true);
  const [showGrid, setShowGrid] = useState<boolean>(false);
  const [show3D, setShow3D] = useState<boolean>(false);
  const [zoomLevel, setZoomLevel] = useState<number>(1);

  // Shared spline path & timing state (editable directly on stage HUD or in Path drawer)
  const [pathPoints, setPathPoints] = useState<Array<[number, number]>>([
    [110, 340],
    [205, 275],
    [305, 325],
  ]);
  const [pathPointIdx, setPathPointIdx] = useState<number>(1);
  const [pathDuration, setPathDuration] = useState<number>(2.5);
  const [sizeStart, setSizeStart] = useState<number>(100);
  const [sizeEnd, setSizeEnd] = useState<number>(100);

  const activePoint = pathPoints[pathPointIdx] || pathPoints[0] || [260, 310];
  const pathX = activePoint[0];
  const pathY = activePoint[1];

  const updatePathCoord = (axis: 0 | 1, val: number) => {
    setPathPoints((prev) =>
      prev.map((pt, i) => (i === pathPointIdx ? (axis === 0 ? [val, pt[1]] : [pt[0], val]) : pt))
    );
  };

  const [selectedSprite, setSelectedSprite] = useState<SpriteAction | null>(null);
  const [activeActionName, setActiveActionName] = useState<string>("sing");
  const [sprites, setSprites] = useState<SpriteAction[]>([]);
  const [framePath, setFramePath] = useState<string>("");
  const [meta, setMeta] = useState<CharacterMetaResponse | null>(null);

  const [isPlaying, setIsPlaying] = useState<boolean>(true);
  const [isLooping, setIsLooping] = useState<boolean>(true);
  const [playbackSpeed, setPlaybackSpeed] = useState<number>(1.0);
  const [motionAmount, setMotionAmount] = useState<number>(100);
  const [currentTime, setCurrentTime] = useState<number>(0);
  const [duration, setDuration] = useState<number>(0.67);

  const [theatreTab, setTheatreTab] = useState<"theatre" | "keys" | "curves">("theatre");

  const [selectedBone, setSelectedBone] = useState<string>("torso");
  const [rotation, setRotation] = useState<number>(0);
  const [scale, setScale] = useState<number>(100);
  const [opacity, setOpacity] = useState<number>(100);

  const [ikActive, setIkActive] = useState<boolean>(true);
  const [limbChain, setLimbChain] = useState<string>("Right Leg (torso → leg_R)");
  const [poleBendClockwise, setPoleBendClockwise] = useState<boolean>(true);
  const [torsoSecondaryFollow, setTorsoSecondaryFollow] = useState<boolean>(true);
  const [ikTargetAngle, setIkTargetAngle] = useState<number>(0);

  const [meshDensity, setMeshDensity] = useState<number>(96);
  const [meshExpansion, setMeshExpansion] = useState<number>(6);
  const [pinMode, setPinMode] = useState<"deform" | "starch" | "overlap">("starch");
  const [pinStiffness, setPinStiffness] = useState<number>(65);
  const [pinExtent, setPinExtent] = useState<number>(38);
  const [followDelay, setFollowDelay] = useState<number>(0.1);

  const [handHold, setHandHold] = useState<boolean>(true);
  const [faceHold, setFaceHold] = useState<boolean>(true);
  const [selectedGesture, setSelectedGesture] = useState<string>("wave");
  const [gestureAngle, setGestureAngle] = useState<number>(0);

  const [placeTrack, setPlaceTrack] = useState<string>("V2");
  const [placeTime, setPlaceTime] = useState<number>(0);
  const [lastRenderedClip, setLastRenderedClip] = useState<string>("");
  const [busy, setBusy] = useState<boolean>(false);

  const selectCharacter = async (charId: string, view = "front") => {
    if (!charId) return;
    setSelectedCharId(charId);
    setSelectedView(view);
    onSelectCharacter?.(charId);
    try {
      const [viewRes, m] = await Promise.all([
        executeCommand<{
          selected?: {
            sprites?: CharacterSpriteSummary[];
            clip?: string;
            frame?: string;
            time?: number;
          };
        }>("animation.view", { character: charId, view }),
        executeCommand<CharacterMetaResponse>("character.meta", { character: charId }),
      ]);

      setMeta(m);
      let spriteList = m?.sprites || viewRes?.selected?.sprites || [];
      const viewObj = m?.views?.find((v) => v.view === view) || m?.views?.[0];

      // If this character has an EffectCraft .ecproj rig on disk but no cached PNG sequence yet,
      // render a 2s preview sequence once via EffectCraft so stage playback works immediately.
      if (spriteList.length === 0 && viewObj?.rigged) {
        try {
          const rendered = await executeCommand<{ frames?: string[] }>("character.render", {
            character: charId,
            view: viewObj.view || view,
            start: 0,
            end: 2.0,
            fps: 16,
            format: "png",
          });
          if (rendered.frames && rendered.frames.length > 0) {
            spriteList = [
              {
                name: "rig-preview",
                ecproj: typeof viewObj.ecproj === "string" ? viewObj.ecproj : "",
                frames: rendered.frames,
                fps: 16,
                duration: 2.0,
              },
            ];
          }
        } catch {
          // Static frame fallback still works below
        }
      }

      setSprites(spriteList);

      if (spriteList.length > 0) {
        const preferred =
          spriteList.find((s) => s.name === "sing") ||
          spriteList.find((s) => s.name === "wave") ||
          spriteList.find((s) => s.name === "jump") ||
          spriteList.find((s) => s.name === "idle") ||
          spriteList[0];
        setSelectedSprite(preferred);
        setActiveActionName(preferred.name);
        setDuration(preferred.duration || 0.67);
      } else {
        setSelectedSprite(null);
        setActiveActionName("rest");
      }

      const sel = viewRes?.selected;
      if (sel) {
        if (sel.clip) setLastRenderedClip(sel.clip);
        if (sel.frame) setFramePath(sel.frame);
      }
      if (!sel?.frame && viewObj?.drawing) {
        setFramePath(viewObj.drawing);
      }
      if (viewObj?.parts && viewObj.parts.length > 0) {
        setSelectedBone(viewObj.parts[0].id);
      }
    } catch {
      // Character rig optional until generated
    }
  };

  useEffect(() => {
    if (characters.length === 0) {
      if (!initialCharacterId) {
        setSelectedCharId("");
        setSprites([]);
        setSelectedSprite(null);
        setFramePath("");
        setMeta(null);
      }
      return;
    }
    const matchId = (cand?: string) =>
      cand
        ? characters.find(
            (c) =>
              c.id === cand ||
              c.name === cand ||
              c.id === cand.replace(/[^a-zA-Z0-9]+/g, "_")
          )?.id
        : undefined;
    const targetId =
      matchId(initialCharacterId) ||
      matchId(selectedCharId) ||
      characters[0].id;
    if (targetId && targetId !== selectedCharId) {
      void selectCharacter(targetId, selectedView);
    } else if (targetId && !meta) {
      void selectCharacter(targetId, selectedView);
    }
  }, [projectPath, initialCharacterId, characters]);

  // Playback timer for smooth frame cycling on the stage
  useEffect(() => {
    if (!isPlaying) return;
    const fps = (selectedSprite?.fps || 24) * playbackSpeed;
    const intervalMs = Math.max(16, Math.round(1000 / Math.max(1, fps)));
    const dt = 1 / Math.max(1, selectedSprite?.fps || 24);
    const timer = window.setInterval(() => {
      setCurrentTime((prev) => {
        const maxDur = Math.max(0.1, selectedSprite?.duration || duration || 1.0);
        const next = prev + dt;
        if (next >= maxDur) {
          return isLooping ? 0 : maxDur;
        }
        return Number(next.toFixed(3));
      });
    }, intervalMs);
    return () => window.clearInterval(timer);
  }, [isPlaying, isLooping, playbackSpeed, selectedSprite, duration]);

  const activeViewSummary =
    meta?.views?.find((v) => v.view === selectedView) || meta?.views?.[0];
  const parts: CharacterPart[] = activeViewSummary?.parts || meta?.parts || [];
  const drawingSize: [number, number] =
    activeViewSummary?.size && activeViewSummary.size.length === 2
      ? activeViewSummary.size
      : [384, 512];

  const bones: BoneNode[] =
    parts.length > 0
      ? parts
          .filter((p) => Array.isArray(p.joint) && p.joint.length === 2)
          .map((p) => ({
            id: p.id,
            label: p.name || p.id,
            x: p.joint![0],
            y: p.joint![1],
            parent: p.parent ?? null,
            outline: p.outline,
          }))
      : DEFAULT_BONES;

  const commitBoneProperty = async (prop: string, val: number) => {
    try {
      await executeCommand("character.set", {
        character: selectedCharId,
        view: selectedView,
        part: selectedBone,
        prop,
        value: val,
        time: currentTime,
      });
      const frameRes = await executeCommand<{ frame?: string }>("character.frame", {
        character: selectedCharId,
        view: selectedView,
        time: currentTime,
      });
      if (frameRes.frame) {
        setFramePath(`${frameRes.frame}?t=${Date.now()}`);
      }
    } catch {
      // Live preview remains responsive
    }
  };

  const handleTriggerAction = async (actionName: string) => {
    setActiveActionName(actionName);
    if (actionName === "rest") {
      setSelectedSprite(null);
      setCurrentTime(0);
      try {
        const frameRes = await executeCommand<{ frame?: string }>("character.frame", {
          character: selectedCharId,
          view: selectedView,
          time: 0,
        });
        if (frameRes.frame) setFramePath(`${frameRes.frame}?t=${Date.now()}`);
      } catch {
        // Use initial drawing if not rigged
      }
      return;
    }

    const matchingSprite = sprites.find((s) => s.name === actionName);
    if (matchingSprite) {
      setSelectedSprite(matchingSprite);
      setDuration(matchingSprite.duration || 0.67);
      setCurrentTime(0);
      return;
    }

    setBusy(true);
    try {
      const res = await executeCommand<{ start?: number; end?: number }>("character.action", {
        character: selectedCharId,
        view: selectedView,
        action: actionName,
      });
      const start = res.start ?? 0;
      const end = res.end ?? start + 1.5;
      setDuration(Math.max(0.5, end - start));
      setCurrentTime(0);
      const frameRes = await executeCommand<{ frame?: string }>("character.frame", {
        character: selectedCharId,
        view: selectedView,
        time: start,
      });
      if (frameRes.frame) setFramePath(`${frameRes.frame}?t=${Date.now()}`);
      onToast(`Keyed action ${actionName} on ${selectedCharId} (${selectedView})`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePrepareArtwork = () => {
    setShowMarks(true);
    const pane = document.getElementById("magic-parts-pane") as HTMLDetailsElement | null;
    if (pane) {
      pane.open = true;
      pane.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  };

  const handleKeyTransform = async () => {
    setBusy(true);
    try {
      await commitBoneProperty("rotation", rotation);
      onToast(`Keyed ${selectedBone}: rot=${rotation}°, scale=${scale}%, opacity=${opacity}%`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSolveIK = async () => {
    setBusy(true);
    try {
      await commitBoneProperty("rotation", ikTargetAngle);
      onToast(`Solved & Keyed 2-Bone IK on ${limbChain} (Target: ${ikTargetAngle}°)`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRenderClip = async () => {
    setBusy(true);
    try {
      const params: Record<string, unknown> = {
        character: selectedCharId,
        view: selectedView,
        start: 0,
        end: duration,
        fps: 24,
        format: "clip",
      };
      if (selectedSprite?.name) {
        params.sprite = selectedSprite.name;
      }
      const res = await executeCommand<{ clip?: string; path?: string }>(
        "character.render",
        params
      );
      const clip = res.clip || res.path || "";
      if (clip) setLastRenderedClip(clip);
      onToast(`Rendered animation clip: ${clip.split("/").pop() || selectedCharId}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePlaceOnTimeline = async () => {
    if (!lastRenderedClip) {
      onToast("Render an animation clip (.mov) first");
      return;
    }
    setBusy(true);
    try {
      await executeCommand("video.place_clip", {
        clip: lastRenderedClip,
        from: placeTime,
        track: placeTrack,
      });
      onToast(`Placed on FilmCraft timeline (${placeTrack} @ ${placeTime}s)`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section
      id="view-animation"
      className="active"
      data-ui-config="view-animation"
      aria-label="Animation workspace"
      style={{ height: "100%", width: "100%" }}
    >
      <AnimationStage
        selectedCharId={selectedCharId}
        selectedView={selectedView}
        selectedSprite={selectedSprite}
        activeActionName={activeActionName}
        sprites={sprites}
        onSelectSprite={(sp) => {
          setSelectedSprite(sp);
          if (sp) {
            setActiveActionName(sp.name);
            setDuration(sp.duration || 0.67);
            setCurrentTime(0);
          } else {
            void handleTriggerAction("rest");
          }
        }}
        showMarks={showMarks}
        onToggleMarks={() => setShowMarks((v) => !v)}
        showPath={showPath}
        onTogglePath={() => setShowPath((v) => !v)}
        showGrid={showGrid}
        onToggleGrid={() => setShowGrid((v) => !v)}
        show3D={show3D}
        onToggle3D={() => setShow3D((v) => !v)}
        zoomLevel={zoomLevel}
        onChangeZoom={setZoomLevel}
        framePath={framePath}
        drawingSize={drawingSize}
        parts={parts}
        bones={bones}
        selectedBone={selectedBone}
        onSelectBone={setSelectedBone}
        pathPoints={pathPoints}
        pathPointIdx={pathPointIdx}
        onSelectPathPoint={setPathPointIdx}
        pathX={pathX}
        onChangePathX={(x) => updatePathCoord(0, x)}
        pathY={pathY}
        onChangePathY={(y) => updatePathCoord(1, y)}
        pathDuration={pathDuration}
        onChangePathDuration={setPathDuration}
        sizeStart={sizeStart}
        onChangeSizeStart={setSizeStart}
        sizeEnd={sizeEnd}
        onChangeSizeEnd={setSizeEnd}
        onClearPath={() => {
          setPathPoints([
            [110, 340],
            [205, 275],
            [305, 325],
          ]);
          setSizeStart(100);
          setSizeEnd(100);
          onToast("Cleared motion path");
        }}
        onUndoPath={() => {
          setPathPoints((prev) =>
            prev.map((pt, i) => (i === pathPointIdx ? [192, 290] : pt))
          );
          onToast(`Reverted Point ${pathPointIdx + 1}`);
        }}
        onApplyPath={() => {
          setDuration(pathDuration);
          onToast(
            `Applied spline path (${pathX}, ${pathY}) · ${pathDuration}s · ${sizeStart}% → ${sizeEnd}%`
          );
        }}
        isPlaying={isPlaying}
        onTogglePlay={() => setIsPlaying((p) => !p)}
        currentTime={currentTime}
        duration={duration}
        onSeek={(t) => {
          setIsPlaying(false);
          setCurrentTime(t);
        }}
        onOpenFullReview={handlePrepareArtwork}
      />

      <AnimationPoseInspector
        selectedCharId={selectedCharId}
        selectedView={selectedView}
        onSelectView={(v) => void selectCharacter(selectedCharId, v)}
        selectedBone={selectedBone}
        onSelectBone={setSelectedBone}
        meta={meta}
        sprites={sprites}
        selectedSprite={selectedSprite}
        onSelectSprite={(sp) => {
          setSelectedSprite(sp);
          if (sp) {
            setActiveActionName(sp.name);
            setDuration(sp.duration || 0.67);
            setCurrentTime(0);
          } else {
            void handleTriggerAction("rest");
          }
        }}
        activeActionName={activeActionName}
        onTriggerAction={(act) => void handleTriggerAction(act)}
        showMarks={showMarks}
        onToggleMarks={() => setShowMarks((v) => !v)}
        onRefreshCharacter={() => selectCharacter(selectedCharId, selectedView)}
        pathPointIdx={pathPointIdx}
        onSelectPathPoint={setPathPointIdx}
        pathX={pathX}
        onChangePathX={(x) => updatePathCoord(0, x)}
        pathY={pathY}
        onChangePathY={(y) => updatePathCoord(1, y)}
        pathDuration={pathDuration}
        onChangePathDuration={setPathDuration}
        sizeStart={sizeStart}
        onChangeSizeStart={setSizeStart}
        sizeEnd={sizeEnd}
        onChangeSizeEnd={setSizeEnd}
        isLooping={isLooping}
        onToggleLoop={setIsLooping}
        playbackSpeed={playbackSpeed}
        onChangePlaybackSpeed={setPlaybackSpeed}
        motionAmount={motionAmount}
        onChangeMotionAmount={setMotionAmount}
        rotation={rotation}
        onChangeRotation={setRotation}
        onCommitRotation={(val) => void commitBoneProperty("rotation", val)}
        scale={scale}
        onChangeScale={setScale}
        onCommitScale={(val) => void commitBoneProperty("scale", val)}
        opacity={opacity}
        onChangeOpacity={setOpacity}
        onCommitOpacity={(val) => void commitBoneProperty("opacity", val)}
        onKeyTransform={() => void handleKeyTransform()}
        ikActive={ikActive}
        onChangeIkActive={setIkActive}
        limbChain={limbChain}
        onChangeLimbChain={setLimbChain}
        poleBendClockwise={poleBendClockwise}
        onChangePoleBend={setPoleBendClockwise}
        torsoSecondaryFollow={torsoSecondaryFollow}
        onChangeTorsoFollow={setTorsoSecondaryFollow}
        ikTargetAngle={ikTargetAngle}
        onChangeIkTargetAngle={setIkTargetAngle}
        onCommitIkTargetAngle={(val) => void commitBoneProperty("rotation", val)}
        onSolveIK={() => void handleSolveIK()}
        meshDensity={meshDensity}
        onChangeMeshDensity={setMeshDensity}
        meshExpansion={meshExpansion}
        onChangeMeshExpansion={setMeshExpansion}
        pinMode={pinMode}
        onChangePinMode={setPinMode}
        pinStiffness={pinStiffness}
        onChangePinStiffness={setPinStiffness}
        pinExtent={pinExtent}
        onChangePinExtent={setPinExtent}
        followDelay={followDelay}
        onChangeFollowDelay={setFollowDelay}
        handHold={handHold}
        onToggleHandHold={() => setHandHold((v) => !v)}
        faceHold={faceHold}
        onToggleFaceHold={() => setFaceHold((v) => !v)}
        selectedGesture={selectedGesture}
        onSelectGesture={setSelectedGesture}
        gestureAngle={gestureAngle}
        onChangeGestureAngle={setGestureAngle}
        onCommitGestureAngle={(val) => void commitBoneProperty("rotation", val)}
        onRenderClip={() => void handleRenderClip()}
        placeTrack={placeTrack}
        onChangePlaceTrack={setPlaceTrack}
        placeTime={placeTime}
        onChangePlaceTime={setPlaceTime}
        lastRenderedClip={lastRenderedClip}
        onPlaceOnTimeline={() => void handlePlaceOnTimeline()}
        busy={busy}
        onToast={onToast}
      />

      <AnimationTheatre
        theatreTab={theatreTab}
        onSelectTheatreTab={setTheatreTab}
        selectedBone={selectedBone}
        onSelectBone={setSelectedBone}
        parts={parts}
        currentTime={currentTime}
        duration={duration}
        onSeek={(t) => {
          setIsPlaying(false);
          setCurrentTime(t);
        }}
        sprites={sprites}
        selectedSprite={selectedSprite}
        activeActionName={activeActionName}
        onSelectSprite={(sp) => {
          setSelectedSprite(sp);
          if (sp) {
            setActiveActionName(sp.name);
            setDuration(sp.duration || 0.67);
            setCurrentTime(0);
          } else {
            void handleTriggerAction("rest");
          }
        }}
        onSaveMotion={() => void handleRenderClip()}
        onToast={onToast}
      />
    </section>
  );
};
