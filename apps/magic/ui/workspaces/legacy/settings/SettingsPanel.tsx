import { useCallback, useEffect, useState } from "react";
import { executeCommand } from "../../../api";
import { Slider, Toggle } from "../../../components/controls";

/**
 * The settings page, as React. First step of the JS → TS/TSX → Rust migration: the smallest
 * legacy page (27 lines of `settings.js`) with no shared globals, its commands (`prefs.schema`,
 * `prefs.set`) already implemented in Rust, and its controls from the app's one vocabulary
 * (`components/controls` → global.css).
 */

type Kind =
  | { type: "bool" }
  | { type: "int" | "float"; min?: number; max?: number; unit?: string }
  | { type: "choice"; choices: Array<{ value: string; label: string }> }
  | { type: "text" }
  | { type: "path" };

interface Field {
  key: string;
  label: string;
  kind: Kind;
  value?: unknown;
  wired?: boolean;
}

interface Category {
  id: string;
  title: string;
  fields: Field[];
}

interface Schema {
  categories: Category[];
}

const numeric = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;
const text = (value: unknown): string => (typeof value === "string" ? value : "");

function FieldControl({ field }: { field: Field }) {
  const [value, setValue] = useState<unknown>(field.value);
  const [state, setState] = useState<"idle" | "saving" | "failed">("idle");

  const save = useCallback(
    async (next: unknown) => {
      setValue(next);
      setState("saving");
      try {
        await executeCommand("prefs.set", { key: field.key, value: next });
        setState("idle");
      } catch {
        setState("failed");
      }
    },
    [field.key],
  );

  const { kind } = field;
  const id = `settings.${field.key}`;
  if (kind.type === "bool") {
    return <Toggle id={id} label={field.label} checked={value === true} onChange={(next) => void save(next)} />;
  }
  if (kind.type === "int" || kind.type === "float") {
    return (
      <Slider
        id={id}
        label={state === "failed" ? `${field.label} · not saved` : field.label}
        value={numeric(value, numeric(kind.min, 0))}
        min={numeric(kind.min, 0)}
        max={numeric(kind.max, 100)}
        step={kind.type === "int" ? 1 : 0.1}
        unit={kind.unit ?? ""}
        onChange={(next) => void save(next)}
      />
    );
  }
  if (kind.type === "choice") {
    return (
      <div className="inspector-field craft-number">
        <label className="craft-number-title" htmlFor={id}>
          <span className="craft-control-label">{field.label}</span>
          <output className="craft-number-readout">{state === "saving" ? "Saving…" : state === "failed" ? "Not saved" : ""}</output>
        </label>
        <span className="craft-number-body">
          <select id={id} className="field" value={text(value)} onChange={(event) => void save(event.currentTarget.value)}>
            {kind.choices.map((choice) => (
              <option key={choice.value} value={choice.value}>{choice.label}</option>
            ))}
          </select>
        </span>
      </div>
    );
  }
  return (
    <div className="inspector-field craft-number">
      <label className="craft-number-title" htmlFor={id}>
        <span className="craft-control-label">{field.label}</span>
        <output className="craft-number-readout">{state === "saving" ? "Saving…" : state === "failed" ? "Not saved" : ""}</output>
      </label>
      <span className="craft-number-body">
        <input id={id} className="field" type="text" value={text(value)} onChange={(event) => void save(event.currentTarget.value)} />
      </span>
    </div>
  );
}

export function SettingsPanel() {
  const [schema, setSchema] = useState<Schema | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    executeCommand<Schema>("prefs.schema")
      .then((reply) => setSchema(reply))
      .catch((reason: unknown) => setError(String(reason)));
  }, []);

  if (error) return <p className="panel-body" data-workspace-error="prefs.schema">{error}</p>;
  if (!schema) return <p className="panel-body">Loading settings…</p>;
  return (
    <>
      {schema.categories.map((category) => (
        <section key={category.id} className="panel-body">
          <h2>{category.title}</h2>
          {category.fields.map((field) => <FieldControl key={field.key} field={field} />)}
        </section>
      ))}
    </>
  );
}
