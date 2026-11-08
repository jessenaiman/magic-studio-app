import { useCallback, useEffect, useState } from "react";
import { executeCommand } from "../../../api";

/**
 * The settings page, as React. First step of the JS → TS/TSX → Rust migration: the smallest
 * legacy page (27 lines of `settings.js`) with no shared globals, its commands (`prefs.schema`,
 * `prefs.set`) already implemented in Rust.
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

function numeric(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

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
  let control;
  if (kind.type === "bool") {
    control = (
      <input
        id={id}
        type="checkbox"
        className="craft-switch"
        checked={value === true}
        onChange={(event) => void save(event.currentTarget.checked)}
      />
    );
  } else if (kind.type === "int" || kind.type === "float") {
    const min = numeric(kind.min, 0);
    const max = numeric(kind.max, 100);
    control = (
      <input
        id={id}
        type="range"
        className="control-range"
        min={min}
        max={max}
        step={kind.type === "int" ? 1 : 0.1}
        value={numeric(value, min)}
        onChange={(event) => void save(Number(event.currentTarget.value))}
      />
    );
  } else if (kind.type === "choice") {
    control = (
      <select id={id} value={text(value)} onChange={(event) => void save(event.currentTarget.value)}>
        {kind.choices.map((choice) => (
          <option key={choice.value} value={choice.value}>{choice.label}</option>
        ))}
      </select>
    );
  } else {
    control = (
      <input
        id={id}
        type="text"
        value={text(value)}
        onChange={(event) => void save(event.currentTarget.value)}
      />
    );
  }

  return (
    <div className="inspector-field craft-number">
      <label htmlFor={id} className="craft-number-title">
        <span className="craft-control-label">{field.label}</span>
        <output className="craft-number-readout">{state === "saving" ? "Saving…" : state === "failed" ? "Not saved" : String(value ?? "")}</output>
      </label>
      <span className="craft-number-body">{control}</span>
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
