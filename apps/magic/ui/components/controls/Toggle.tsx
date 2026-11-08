/** A switch from the app's control vocabulary: `.craft-switch` inside `.inspector-field`. */
interface ToggleProps {
  id: string;
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

export function Toggle({ id, label, checked, disabled = false, onChange }: ToggleProps) {
  return (
    <label className="inspector-field field-row craft-toggle" htmlFor={id}>
      <input
        id={id}
        className="craft-switch"
        type="checkbox"
        role="switch"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span className="craft-control-label">{label}</span>
    </label>
  );
}
