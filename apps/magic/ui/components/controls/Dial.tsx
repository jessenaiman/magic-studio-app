import { useRef } from "react";

/**
 * The SVG dial from `global.css` (`.craft-dial` track/face/marker), mirroring the legacy
 * `InspectorNumberField.createDial` interaction: drag up/down, Shift for fine, arrows to step.
 */
interface DialProps {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  disabled?: boolean;
  onChange: (value: number) => void;
}

export function Dial({ id, label, value, min, max, step = 1, unit = "", disabled = false, onChange }: DialProps) {
  const start = useRef<{ y: number; value: number } | null>(null);
  const clamp = (next: number) => Math.min(max, Math.max(min, Number(next.toFixed(6))));
  const fraction = (value - min) / (max - min || 1);
  const readout = `${Number(value.toFixed(2))}${unit}`;

  return (
    <span
      id={`${id}-dial`}
      className="craft-dial"
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={`${label} dial`}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={readout}
      aria-disabled={disabled}
      title="Drag up or down · Shift for fine adjustment · arrow keys to step"
      onPointerDown={(event) => {
        if (disabled) return;
        event.preventDefault();
        start.current = { y: event.clientY, value };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        const from = start.current;
        if (!from || disabled) return;
        onChange(clamp(from.value + (from.y - event.clientY) * step * (event.shiftKey ? 0.1 : 1)));
      }}
      onPointerUp={() => { start.current = null; }}
      onPointerCancel={() => { start.current = null; }}
      onKeyDown={(event) => {
        const delta = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1, PageUp: 10, PageDown: -10 }[event.key] ?? 0;
        if (!delta || disabled) return;
        event.preventDefault();
        onChange(clamp(value + delta * step * (event.shiftKey ? 10 : 1)));
      }}
    >
      <svg viewBox="0 0 48 48" aria-hidden="true">
        <circle className="craft-dial-track" cx="24" cy="24" r="19" />
        <circle className="craft-dial-face" cx="24" cy="24" r="14" />
        <path className="craft-dial-marker" d="M24 13v7" transform={`rotate(${-135 + 270 * fraction} 24 24)`} />
      </svg>
    </span>
  );
}
