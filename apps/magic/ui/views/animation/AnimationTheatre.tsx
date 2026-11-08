import React from "react";
import { CharacterPart } from "../../api";
import { SpriteAction } from "./AnimationStage";

interface AnimationTheatreProps {
  theatreTab: "theatre" | "keys" | "curves";
  onSelectTheatreTab: (tab: "theatre" | "keys" | "curves") => void;
  selectedBone: string;
  onSelectBone: (boneId: string) => void;
  parts: CharacterPart[];
  currentTime: number;
  duration: number;
  onSeek: (time: number) => void;
  sprites: SpriteAction[];
  selectedSprite: SpriteAction | null;
  activeActionName: string;
  onSelectSprite: (sprite: SpriteAction | null) => void;
  onSaveMotion: () => void;
  onToast: (msg: string) => void;
}

export const AnimationTheatre: React.FC<AnimationTheatreProps> = ({
  theatreTab,
  onSelectTheatreTab,
  selectedBone,
  onSelectBone,
  parts,
  currentTime,
  duration,
  onSeek,
  sprites,
  selectedSprite,
  activeActionName,
  onSelectSprite,
  onSaveMotion,
  onToast,
}) => {
  const playheadAt = duration > 0 ? Math.max(0, Math.min(1, currentTime / duration)) : 0;
  const channelList =
    parts.length > 0
      ? parts.map((p) => ({ id: p.id, label: p.name || p.id, parent: p.parent || "root" }))
      : [
          { id: "torso", label: "torso", parent: "root" },
          { id: "head", label: "head", parent: "torso" },
          { id: "arm_L", label: "arm_L", parent: "torso" },
          { id: "arm_R", label: "arm_R", parent: "torso" },
          { id: "leg_L", label: "leg_L", parent: "torso" },
          { id: "leg_R", label: "leg_R", parent: "torso" },
        ];

  return (
    <section
      className="motion-theatre"
      data-view={theatreTab === "theatre" ? "curves" : theatreTab}
      aria-label="Theatre timeline"
    >
      <div id="theatre-bar" className="theatre-heading">
        <span className="theatre-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M3 3h18v18H3zM3 9h18M3 15h18M9 3v18M15 3v18" />
          </svg>
        </span>
        <strong>Theatre</strong>
        <div className="theatre-views segmented" role="tablist" aria-label="Theatre view">
          {(["theatre", "keys", "curves"] as const).map((t) => (
            <button
              key={t}
              id={`theatre-view-${t}`}
              type="button"
              role="tab"
              aria-selected={theatreTab === t}
              className={theatreTab === t ? "active" : ""}
              onClick={() => onSelectTheatreTab(t)}
            >
              {t === "theatre" ? "Theatre" : t === "keys" ? "Keys" : "Curves"}
            </button>
          ))}
        </div>
        <span id="theatre-info" className="theatre-info muted">
          REVIEW RANGE 0.0s–{duration.toFixed(1)}s
        </span>
        <div className="theatre-actions">
          <span id="theatre-status" className="muted">
            {selectedBone} · Save motion to persist edits
          </span>
          <button
            id="theatre-save"
            type="button"
            className="ghost anim-icon-btn accent"
            onClick={onSaveMotion}
          >
            Save motion
          </button>
        </div>
      </div>

      <div className="theatre-cliprow">
        <span className="theatre-label muted">CLIPS</span>
        <div id="theatre-clips" className="theatre-clips" aria-label="Authored clips">
          <button
            type="button"
            className={`theatre-clip-pill ${!selectedSprite && activeActionName === "rest" ? "active" : ""}`}
            aria-pressed={!selectedSprite && activeActionName === "rest"}
            onClick={() => onSelectSprite(null)}
          >
            Rest
          </button>
          {sprites.map((s) => {
            const active = selectedSprite?.name === s.name;
            return (
              <button
                key={s.name}
                type="button"
                className={`theatre-clip-pill ${active ? "active" : ""}`}
                aria-pressed={active}
                onClick={() => onSelectSprite(s)}
              >
                {s.name}
              </button>
            );
          })}
        </div>
        <div className="theatre-actions">
          <button
            type="button"
            className="ghost anim-icon-btn"
            onClick={() => onToast("Created section from selection")}
          >
            Create section from selection
          </button>
          <button
            type="button"
            className="ghost anim-icon-btn"
            onClick={() => onToast("Custom actions opened")}
          >
            Custom actions
          </button>
          <button
            type="button"
            className="ghost anim-icon-btn"
            onClick={() => onToast("Design as actions")}
          >
            Design as actions
          </button>
        </div>
      </div>

      <div className="theatre-body">
        <div className="theatre-curves">
          <div className="theatre-grid">
            <div className="panel-head" data-look="ruler">
              <span className="theatre-channel muted">Channel</span>
              <span className="theatre-motion muted">Motion</span>
              <div
                className="theatre-ruler"
                onClick={(e) => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  const frac = Math.max(0, Math.min(1, (e.clientX - rect.left) / (rect.width || 1)));
                  onSeek(Number((frac * duration).toFixed(3)));
                }}
              >
                {[0, 0.25, 0.5, 0.75, 1].map((tick) => (
                  <span
                    key={tick}
                    style={{ "--tick-position": `${tick * 100}%` } as React.CSSProperties}
                  >
                    {(tick * duration).toFixed(1)}s
                  </span>
                ))}
              </div>
            </div>

            {channelList.map((ch, idx) => {
              const isSel = selectedBone === ch.id;
              const phase = idx * 0.6;
              return (
                <div
                  key={ch.id}
                  className={`theatre-row ${isSel ? "selected" : ""}`}
                  style={
                    {
                      "--tone": isSel ? "var(--amber)" : "var(--motion-blue)",
                    } as React.CSSProperties
                  }
                  onClick={() => onSelectBone(ch.id)}
                >
                  <button
                    type="button"
                    className="theatre-channel"
                    onClick={() => onSelectBone(ch.id)}
                  >
                    {ch.label}
                  </button>
                  <span className="theatre-motion">
                    {selectedSprite ? `${selectedSprite.name} (${selectedSprite.fps || 24}fps)` : activeActionName}
                  </span>
                  <svg
                    className="theatre-curve"
                    viewBox="0 0 200 22"
                    preserveAspectRatio="none"
                    onClick={(e) => {
                      const rect = e.currentTarget.getBoundingClientRect();
                      const frac = Math.max(
                        0,
                        Math.min(1, (e.clientX - rect.left) / (rect.width || 1))
                      );
                      onSeek(Number((frac * duration).toFixed(3)));
                    }}
                  >
                    <path
                      d={`M 0 11 Q 50 ${11 - Math.sin(phase + 1) * 7}, 100 11 T 200 11`}
                      fill="none"
                      stroke={isSel ? "var(--amber)" : "var(--motion-blue)"}
                      strokeWidth="1.6"
                    />
                    {[0.125, 0.375, 0.625, 0.875].map((kt) => (
                      <circle
                        key={kt}
                        cx={kt * 200}
                        cy={11 - Math.sin(phase + kt * Math.PI * 2) * 5}
                        r={isSel ? 3 : 2.2}
                        fill={isSel ? "var(--amber)" : "var(--accent)"}
                      />
                    ))}
                  </svg>
                </div>
              );
            })}

            <div
              className="theatre-playhead"
              style={{ "--at": playheadAt } as React.CSSProperties}
            />
          </div>

          <div className="theatre-foot muted">
            <span>
              {selectedSprite
                ? `Active cycle: ${selectedSprite.name} · ${selectedSprite.frames?.length || 8} frames`
                : `Active rig view: ${activeActionName}`}
            </span>
            <span>Click ruler or curve to scrub · Select channel to inspect in Expression &amp; pose</span>
          </div>
        </div>
      </div>
    </section>
  );
};
