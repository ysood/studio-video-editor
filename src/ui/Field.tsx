import { useEffect, useState, type ReactNode } from "react";
import "./field.css";

/** A labelled row: label on the left, control on the right. */
export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="ui-field">
      <span className="ui-field-label">{label}</span>
      {children}
    </label>
  );
}

/** Read-only value shown in a Field. */
export function Value({ children, numeric, truncate = "end", title }: {
  children: ReactNode;
  numeric?: boolean;
  truncate?: "end" | "start";
  title?: string;
}) {
  return (
    <span className={numeric ? "ui-value numeric" : "ui-value"} data-truncate={truncate} title={title}>
      {children}
    </span>
  );
}

interface CommitProps {
  value: string;
  onCommit: (value: string) => void;
  disabled?: boolean;
  ariaLabel?: string;
}

/** Text input that commits on Enter or blur and reverts on Escape. */
export function TextInput({ value, onCommit, disabled, ariaLabel, numeric }: CommitProps & { numeric?: boolean }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      className={numeric ? "ui-input numeric" : "ui-input"}
      type={numeric ? "number" : "text"}
      value={draft}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => draft !== value && onCommit(draft)}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          setDraft(value);
          e.currentTarget.blur();
        }
      }}
    />
  );
}

/** Integer input built on TextInput; non-numbers are ignored. */
export function NumberInput({ value, onCommit, disabled, ariaLabel }: {
  value: number;
  onCommit: (value: number) => void;
  disabled?: boolean;
  ariaLabel?: string;
}) {
  return (
    <TextInput
      numeric
      value={String(value)}
      disabled={disabled}
      ariaLabel={ariaLabel}
      onCommit={(text) => {
        const n = Math.round(Number(text));
        if (Number.isFinite(n)) onCommit(n);
      }}
    />
  );
}

export interface SelectOption {
  value: string;
  label: string;
}

export function Select({ value, options, onChange, disabled, ariaLabel }: {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  ariaLabel?: string;
}) {
  return (
    <select
      className="ui-input"
      value={value}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={(e) => onChange(e.target.value)}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Checkbox({ checked, onChange, disabled, ariaLabel }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  ariaLabel?: string;
}) {
  return (
    <input
      className="ui-checkbox"
      type="checkbox"
      checked={checked}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={(e) => onChange(e.target.checked)}
    />
  );
}

export function Slider({ value, min, max, step, onChange, ariaLabel, className }: {
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  ariaLabel: string;
  className?: string;
}) {
  return (
    <input
      className={className ? `ui-slider ${className}` : "ui-slider"}
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      aria-label={ariaLabel}
      onChange={(e) => onChange(Number(e.target.value))}
    />
  );
}
