/**
 * A labelled range control from the app's one control vocabulary (`global.css`):
 * `.craft-number` + `.control-range` + `.craft-number-readout`, the same markup the legacy
 * `InspectorNumberField` renders. Panels use this instead of raw `<input type="range">`.
 */
import React, { useRef } from "react";
import { Dial } from "./Dial";

/**
 * A labelled range control from the app's one control vocabulary (`global.css`):
 * `.craft-number` + `.control-range` + `.craft-number-readout`, the same markup the legacy
 * `InspectorNumberField` renders. Panels use this instead of raw `<input type="range">`.
 */
interface SliderProps {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  control?: "horizontal" | "vertical" | "dial";
  disabled?: boolean;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
}

export function Slider({
  id,
  label,
  value,
  min,
  max,
  step = 1,
  unit = "",
  control = "horizontal",
  disabled = false,
  onChange,
  onCommit,
}: SliderProps) {
  const dragOrigin = useRef<number | null>(null);
  const fillPct = Math.max(0, Math.min(100, ((value - min) / (max - min || 1)) * 100));
  return (
    <div
      className="inspector-field craft-number"
      data-control={control}
      data-disabled={disabled ? "true" : undefined}
    >
      <label className="craft-number-title" htmlFor={id}>
        <span className="craft-control-label">{label}</span>
        <output id={`${id}-value`} htmlFor={id} className="craft-number-readout">
          {Number(value.toFixed(2))}{unit}
        </output>
      </label>
      <span className="craft-number-body">
        {control === "dial" && (
          <Dial
            id={id}
            label={label}
            value={value}
            min={min}
            max={max}
            step={step}
            unit={unit}
            disabled={disabled}
            onChange={onChange}
            onCommit={onCommit}
          />
        )}
        <input
          id={id}
          className="control-range"
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          aria-label={label}
          aria-orientation={control === "vertical" ? "vertical" : "horizontal"}
          style={{ "--range-fill": `${fillPct}%` } as React.CSSProperties}
          onPointerDown={() => {
            dragOrigin.current = value;
          }}
          onChange={(event) => onChange(Number(event.currentTarget.value))}
          onPointerUp={(event) => {
            const next = Number(event.currentTarget.value);
            if (dragOrigin.current !== null && dragOrigin.current !== next) {
              onCommit?.(next);
            }
            dragOrigin.current = null;
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape" && dragOrigin.current !== null) {
              event.preventDefault();
              const initial = dragOrigin.current;
              dragOrigin.current = null;
              onChange(initial);
            }
          }}
          onKeyUp={(event) => {
            if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "PageUp", "PageDown", "Enter"].includes(event.key)) {
              onCommit?.(Number(event.currentTarget.value));
            }
          }}
        />
      </span>
    </div>
  );
}
