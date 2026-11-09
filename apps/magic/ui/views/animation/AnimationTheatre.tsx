import React from "react";
import { SpriteAction } from "./AnimationStage";

type KeyValue = number | number[];

export interface RigKeyChannel {
  path: string;
  name: string;
  type: string;
  keys: Array<{ time: number; value: KeyValue; in: string; out: string }>;
  samples: Array<{ time: number; value: KeyValue }>;
}

export interface RigKeyLayer {
  id: number;
  name: string;
  parent: string | null;
  channels: RigKeyChannel[];
}

/** The reply of `character.keys`. `start` and `end` are null when the action has no keys. */
export interface RigKeys {
  comp: string;
  fps: number;
  action: string | null;
  start: number | null;
  end: number | null;
  layers: RigKeyLayer[];
}

const components = (value: KeyValue): number[] => (Array.isArray(value) ? value : [value]);

/** Maps a channel's values into the 22-unit row, using the component that moves the most. */
function rowScale(channel: RigKeyChannel) {
  const count = Math.max(1, ...channel.samples.map((s) => components(s.value).length));
  let axis = 0;
  let widest = -1;
  for (let i = 0; i < count; i += 1) {
    const values = channel.samples.map((s) => components(s.value)[i] ?? 0);
    const range = Math.max(...values) - Math.min(...values);
    if (range > widest) {
      widest = range;
      axis = i;
    }
  }
  const values = channel.samples.map((s) => components(s.value)[axis] ?? 0);
  const min = Math.min(...values);
  const range = Math.max(...values) - min;
  return (value: KeyValue) => (range > 0 ? 19 - (((components(value)[axis] ?? 0) - min) / range) * 16 : 11);
}

interface AnimationTheatreProps {
  theatreTab: "theatre" | "keys" | "curves";
  onSelectTheatreTab: (tab: "theatre" | "keys" | "curves") => void;
  selectedBone: string;
  onSelectBone: (boneId: string) => void;
  rigKeys: RigKeys | null;
  keysError: string;
  isSpriteCycle: boolean;
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
  rigKeys,
  keysError,
  isSpriteCycle,
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
  const layers = isSpriteCycle ? [] : (rigKeys?.layers ?? []);
  const windowStart = rigKeys?.start ?? 0;
  const windowSpan = Math.max((rigKeys?.end ?? 0) - windowStart, 1e-6);
  const xOf = (time: number) => ((time - windowStart) / windowSpan) * 200;
  const emptyMessage = isSpriteCycle
    ? "2D sprite cycle: drawn frames, so there are no bone keys."
    : keysError || "No rig keys for this character.";

  return (
    <section
      className="motion-theatre"
      data-view="curves"
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

            {layers.length === 0 && (
              <div className="panel-empty">
                <p className="muted">{emptyMessage}</p>
              </div>
            )}

            {layers.map((layer) => {
              const isSel = selectedBone === layer.name;
              const channel = layer.channels.find((c) => c.path === "transform/rotation") ?? layer.channels[0];
              const yOf = channel ? rowScale(channel) : null;
              return (
                <div
                  key={layer.id}
                  className={`theatre-row ${isSel ? "selected" : ""}`}
                  style={
                    {
                      "--tone": isSel ? "var(--amber)" : "var(--motion-blue)",
                    } as React.CSSProperties
                  }
                  onClick={() => onSelectBone(layer.name)}
                >
                  <button
                    type="button"
                    className="theatre-channel"
                    onClick={() => onSelectBone(layer.name)}
                  >
                    {layer.name}
                  </button>
                  <span className="theatre-motion">
                    {channel ? `${channel.name} · ${channel.keys.length} keys` : "no keys"}
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
                    {channel && yOf && theatreTab !== "keys" && (
                      <polyline
                        points={channel.samples.map((s) => `${xOf(s.time)},${yOf(s.value)}`).join(" ")}
                        fill="none"
                        stroke={isSel ? "var(--amber)" : "var(--motion-blue)"}
                        strokeWidth="1.6"
                      />
                    )}
                    {channel &&
                      yOf &&
                      channel.keys.map((key) => (
                        <circle
                          key={key.time}
                          cx={xOf(key.time)}
                          cy={yOf(key.value)}
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
