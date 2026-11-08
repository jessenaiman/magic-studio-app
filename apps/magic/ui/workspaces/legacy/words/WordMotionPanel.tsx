import { useCallback, useEffect, useMemo, useState } from "react";
import { StyleClipboard } from "../../../components/StyleClipboard";
import { Dial, Slider, Toggle } from "../../../components/controls";
import type { MotionProperty, MotionSetReply, NativeMotion, PropertiesReply, WordItem, WordsAnimation } from "./legacy";

/**
 * Word motion, as React. Ported from `apps/magic-cli/ui/native-word-controls.js`.
 *
 * `lettering/lyrics.ecproj` is authoritative: Apply calls `words.motion.set`, which writes the
 * transform keyframes on the real lyrics layer, saves the EffectCraft project and records only
 * the comp/layer binding (plus a read-back of the keys) in words.json.
 */

const ROTATION = "transform/rotation";
const OPACITY = "transform/opacity";
const POSITION = "transform/position";
const SCALE = "transform/scale";

const FALLBACK_PROPERTIES: MotionProperty[] = [
  { path: POSITION, name: "Position" },
  { path: ROTATION, name: "Rotation" },
  { path: SCALE, name: "Scale" },
  { path: OPACITY, name: "Opacity" },
];

type Pair = [number, number];

interface MotionSpec {
  property: string;
  delay: number;
  duration: number;
  from: number | Pair;
  to: number | Pair;
  speed: number;
}

const finite = (value: unknown, fallback: number): number => (typeof value === "number" && Number.isFinite(value) ? value : fallback);
const pair = (value: unknown, fallback: Pair): Pair =>
  Array.isArray(value) && value.length >= 2 ? [finite(value[0], fallback[0]), finite(value[1], fallback[1])] : fallback;
const scalar = (value: unknown, fallback: number): number => (typeof value === "number" && Number.isFinite(value) ? value : fallback);
const wordIdsKey = (item: WordItem | undefined): string => (item?.word_ids ?? []).join("\u0001");
const cellKey = (anim: WordsAnimation, lineId: string | null, item: WordItem | undefined): string =>
  `${anim.animation_id}\u0001${lineId ?? ""}\u0001${wordIdsKey(item)}`;

/** Keep the last applied values while the page stays open; words.json no longer stores the config. */
const lastMotion = new Map<string, MotionSpec>();

function defaults(property: string): MotionSpec {
  if (property === POSITION) return { property, delay: 0, duration: 2, from: [0, 0], to: [0, 0], speed: 0 };
  if (property === SCALE) return { property, delay: 0, duration: 2, from: [100, 100], to: [100, 100], speed: 0 };
  if (property === OPACITY) return { property, delay: 0, duration: 2, from: 100, to: 100, speed: 0 };
  return { property, delay: 0, duration: 2, from: 0, to: 0, speed: 0 };
}

function fromNativeMotion(native: NativeMotion | null | undefined, item: WordItem): MotionSpec {
  const property = typeof native?.property === "string" ? native.property : ROTATION;
  const spec = defaults(property);
  if (!native) return spec;
  spec.delay = finite(native.delay, spec.delay);
  spec.duration = finite(native.duration, spec.duration);
  if (property === POSITION || property === SCALE) {
    const fallback = property === SCALE ? ([100, 100] as Pair) : ([0, 0] as Pair);
    spec.from = pair(native.from, fallback);
    spec.to = pair(native.to, fallback);
  } else {
    spec.from = scalar(native.from, property === OPACITY ? 100 : 0);
    spec.to = scalar(native.to, property === OPACITY ? 100 : 0);
  }
  spec.speed = finite(native.speed, 0);
  // An item that already carries keys but no config still needs a usable duration.
  if (!native.duration && item.motion_keys?.length) spec.duration = Math.max(0.05, finite(item.motion_keys[item.motion_keys.length - 1]?.[0], 2));
  return spec;
}

function DialField({
  id,
  label,
  value,
  min,
  max,
  step,
  unit,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  unit?: string;
  onChange: (value: number) => void;
}) {
  return (
    <div className="craft-number" data-control="dial">
      <label className="craft-number-title" htmlFor={`${id}-dial`}>
        <span className="craft-control-label">{label}</span>
        <output className="craft-number-readout">
          {Number(value.toFixed(2))}
          {unit ?? ""}
        </output>
      </label>
      <span className="craft-number-body">
        <Dial id={id} label={label} value={value} min={min} max={max} step={step} unit={unit} onChange={onChange} />
      </span>
    </div>
  );
}

export function WordMotionPanel({ anim }: { anim: WordsAnimation }) {
  const lineId = wordsPanel.line;
  const design = useMemo(() => (lineId ? wordsDesign(anim, lineId) : { items: [], draws: [] }), [anim, lineId]);
  const index = design.items.length ? wordsStageController.resolveActiveItemIndex(design, wordsPanel.picked ?? []) : -1;
  const item = design.items[index];
  const words = lineId ? wordsLineWords(anim, lineId) : [];
  const label = (item?.word_ids ?? []).map((id) => words.find((word) => word.word_id === id)?.text).filter(Boolean).join(" ");
  const key = cellKey(anim, lineId, item);

  const [properties, setProperties] = useState<MotionProperty[]>(FALLBACK_PROPERTIES);
  const [profile, setProfile] = useState<PropertiesReply | null>(null);
  const [spec, setSpec] = useState<MotionSpec>(() => lastMotion.get(key) ?? fromNativeMotion(item?.native_motion, item ?? { word_ids: [] }));
  const [state, setState] = useState<"loading" | "ready" | "saving" | "error">("loading");
  const [message, setMessage] = useState("Reading lettering/lyrics.ecproj…");

  useEffect(() => {
    let live = true;
    if (!item) {
      setState("ready");
      setMessage("Select a word group to set its motion.");
      return () => {
        live = false;
      };
    }
    setState("loading");
    setMessage("Reading lettering/lyrics.ecproj…");
    wordsCommand<PropertiesReply>("words.motion.properties", { line_id: lineId, word_ids: item.word_ids })
      .then((reply) => {
        if (!live) return;
        const found = reply.properties?.length ? reply.properties : FALLBACK_PROPERTIES;
        setProperties(found);
        setProfile(reply);
        const animated = found.filter((candidate) => Array.isArray(candidate.keys) && candidate.keys.length >= 2);
        if (!found.some((candidate) => candidate.path === spec.property)) setSpec(defaults(found[0]?.path ?? ROTATION));
        // The ecproj is authoritative: when exactly one transform channel carries keys, open
        // the panel on that channel instead of the rotation default.
        else if (!lastMotion.has(key) && !item.native_motion && animated.length === 1 && animated[0].path !== spec.property)
          setSpec({ ...defaults(animated[0].path), delay: spec.delay, duration: spec.duration });
        setState("ready");
        setMessage(
          reply.binding
            ? `Bound to ${reply.binding.layer?.name ?? "the lyrics layer"} · ${reply.comp?.name ?? "Lyrics"}`
            : "Choose a property. Apply writes keyframes to lettering/lyrics.ecproj.",
        );
      })
      .catch((error: unknown) => {
        if (!live) return;
        setState("error");
        setMessage(`Native controls unavailable: ${String(error)}`);
      });
    return () => {
      live = false;
    };
    // The panel is mounted per legacy render; the word group and line identify the request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [anim.animation_id, lineId, wordIdsKey(item)]);

  const update = useCallback((next: MotionSpec) => setSpec(next), []);
  const updatePair = (field: "from" | "to", axis: 0 | 1, value: number) => {
    const current = pair(spec[field], [0, 0]);
    const next: Pair = [current[0], current[1]];
    next[axis] = value;
    update({ ...spec, [field]: next });
  };

  const apply = useCallback(async () => {
    if (!item || !lineId) return;
    setState("saving");
    setMessage("Writing keyframes to lettering/lyrics.ecproj…");
    try {
      const values =
        spec.property === POSITION || spec.property === SCALE
          ? { from: pair(spec.from, [0, 0]), to: pair(spec.to, [0, 0]) }
          : { from: scalar(spec.from, 0), to: scalar(spec.to, 0) };
      const request: Record<string, unknown> = {
        animation_id: anim.animation_id,
        line_id: lineId,
        word_ids: item.word_ids,
        expected_source_revision: finite(anim.source_revision, 0),
        property: spec.property,
        delay: spec.delay,
        duration: spec.duration,
        from: values.from,
      };
      if (spec.property === ROTATION) request.speed = spec.speed;
      else request.to = values.to;
      const reply = await wordsCommand<MotionSetReply>("words.motion.set", request);
      if (!reply.ok) throw new Error(reply.reason ?? "The animation changed; reload and reapply the motion");
      lastMotion.set(key, spec);
      const animations = wordsPanel.read?.document?.animations;
      if (Array.isArray(animations) && reply.animation) {
        const at = animations.findIndex((candidate) => candidate.animation_id === anim.animation_id);
        if (at >= 0) animations[at] = reply.animation;
      }
      // Keep any unsaved design draft, but move its motion to the keys the ecproj just
      // recorded. The saved animation above advances the source revision so Save still works.
      const draft = wordsPanel.designs.get(lineId);
      const target = draft?.items.find((candidate) => candidate.word_ids.join("\u0001") === item.word_ids.join("\u0001"));
      if (target) {
        target.motion_keys = reply.motion_keys;
        target.native_motion_binding = reply.binding;
        target.accent = item.word_ids[0];
        delete target.native_motion;
      }
      wordsPanel.item = index;
      wordsPanel.picked = [...item.word_ids];
      toast(`Word motion saved · ${reply.motion_keys?.length ?? 0} keyframes in lyrics.ecproj`);
      wordsRender();
    } catch (error: unknown) {
      setState("error");
      setMessage(String(error));
    }
  }, [anim, index, item, key, lineId, spec]);

  if (!item) {
    return (
      <section id="native-word-controls" className="panel-body word-motion-panel" aria-label="Native word motion controls">
        <strong>Word motion</strong>
        <p id="native-word-motion-status" className="muted" role="status">
          Select a word group first.
        </p>
      </section>
    );
  }

  const property = spec.property;
  const disabled = state === "saving" || state === "loading";
  return (
    <section id="native-word-controls" className="panel-body word-motion-panel" aria-label="Native word motion controls">
      <strong>Word motion · {label || item.word_ids.join(" ")}</strong>
      <StyleClipboard
        id="word-style-clipboard"
        title="Word style"
        design={design}
        itemIndex={index}
        onChange={(next) =>
          wordsDesignEdit(anim, lineId ?? "", (draft) => {
            draft.items = next.items as typeof draft.items;
            draft.draws = next.draws as typeof draft.draws;
          })
        }
        onToast={toast}
      />
      <div className="inspector-field craft-number">
        <label className="craft-number-title" htmlFor="word-motion-property">
          <span className="craft-control-label">Property</span>
          <output className="craft-number-readout">{profile?.comp?.name ?? ""}</output>
        </label>
        <span className="craft-number-body">
          <select
            id="word-motion-property"
            className="field"
            value={property}
            disabled={disabled}
            onChange={(event) => {
              const next = defaults(event.currentTarget.value);
              next.delay = spec.delay;
              next.duration = spec.duration;
              update(next);
            }}
          >
            {properties.map((candidate) => (
              <option key={candidate.path} value={candidate.path}>
                {candidate.name}
              </option>
            ))}
          </select>
        </span>
      </div>
      <div className="native-motion-fields">
        <Slider id="word-motion-delay" label="Delay after word · seconds" value={spec.delay} min={0} max={7.95} step={0.05} disabled={disabled} onChange={(value) => update({ ...spec, delay: value })} />
        <DialField
          id="word-motion-duration"
          label="Duration · seconds"
          value={spec.duration}
          min={0.05}
          max={8}
          step={0.05}
          unit="s"
          onChange={(value) => update({ ...spec, duration: value })}
        />
        {property === ROTATION ? (
          <>
            <DialField
              id="word-motion-from"
              label="Start angle offset · degrees"
              value={scalar(spec.from, 0)}
              min={-360}
              max={360}
              step={1}
              unit="°"
              onChange={(value) => update({ ...spec, from: value })}
            />
            <Slider
              id="word-motion-speed"
              label="Rotation speed · degrees / second"
              value={spec.speed}
              min={-1800}
              max={1800}
              step={5}
              disabled={disabled}
              onChange={(value) => update({ ...spec, speed: value })}
            />
            <Toggle
              id="word-motion-reverse"
              label="Reverse direction"
              checked={spec.speed < 0}
              disabled={disabled}
              onChange={(checked) => update({ ...spec, speed: checked ? -Math.max(5, Math.abs(spec.speed)) : Math.abs(spec.speed) })}
            />
          </>
        ) : property === OPACITY ? (
          <>
            <Slider id="word-motion-from" label="From · %" value={scalar(spec.from, 100)} min={0} max={100} step={1} disabled={disabled} onChange={(value) => update({ ...spec, from: value })} />
            <Slider id="word-motion-to" label="To · %" value={scalar(spec.to, 100)} min={0} max={100} step={1} disabled={disabled} onChange={(value) => update({ ...spec, to: value })} />
          </>
        ) : (
          <>
            <Slider
              id="word-motion-from-0"
              label={`From X · ${property === SCALE ? "%" : "px offset"}`}
              value={pair(spec.from, [0, 0])[0]}
              min={property === SCALE ? 1 : -1920}
              max={property === SCALE ? 400 : 1920}
              step={1}
              disabled={disabled}
              onChange={(value) => updatePair("from", 0, value)}
            />
            <Slider
              id="word-motion-from-1"
              label={`From Y · ${property === SCALE ? "%" : "px offset"}`}
              value={pair(spec.from, [0, 0])[1]}
              min={property === SCALE ? 1 : -1080}
              max={property === SCALE ? 400 : 1080}
              step={1}
              disabled={disabled}
              onChange={(value) => updatePair("from", 1, value)}
            />
            <Slider
              id="word-motion-to-0"
              label={`To X · ${property === SCALE ? "%" : "px offset"}`}
              value={pair(spec.to, [0, 0])[0]}
              min={property === SCALE ? 1 : -1920}
              max={property === SCALE ? 400 : 1920}
              step={1}
              disabled={disabled}
              onChange={(value) => updatePair("to", 0, value)}
            />
            <Slider
              id="word-motion-to-1"
              label={`To Y · ${property === SCALE ? "%" : "px offset"}`}
              value={pair(spec.to, [0, 0])[1]}
              min={property === SCALE ? 1 : -1080}
              max={property === SCALE ? 400 : 1080}
              step={1}
              disabled={disabled}
              onChange={(value) => updatePair("to", 1, value)}
            />
          </>
        )}
      </div>
      <p id="native-word-motion-status" className="muted" role="status">
        {message}
      </p>
      <div className="words-selection-actions">
        <button id="word-motion-apply" className="ghost" type="button" disabled={disabled} onClick={() => void apply()}>
          Apply motion to selection
        </button>
      </div>
    </section>
  );
}
