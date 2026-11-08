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
  disabled?: boolean;
  onChange: (value: number) => void;
}

export function Slider({ id, label, value, min, max, step = 1, unit = "", disabled = false, onChange }: SliderProps) {
  const fillPct = Math.max(0, Math.min(100, ((value - min) / (max - min || 1)) * 100));
  return (
    <div className="craft-number" data-disabled={disabled ? "true" : undefined}>
      <label className="craft-number-title" htmlFor={id}>
        <span className="craft-control-label">{label}</span>
        <output className="craft-number-readout">{Number(value.toFixed(2))}{unit}</output>
      </label>
      <span className="craft-number-body">
        <input
          id={id}
          className="control-range"
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          style={{ "--range-fill": `${fillPct}%` } as React.CSSProperties}
          onChange={(event) => onChange(Number(event.currentTarget.value))}
        />
      </span>
    </div>
  );
}
