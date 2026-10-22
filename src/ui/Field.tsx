import * as SelectPrimitive from "@radix-ui/react-select";
import * as SliderPrimitive from "@radix-ui/react-slider";
import { useEffect, useState, type ReactNode } from "react";
import { Icon } from "./Icon";
import { Row } from "./List";
import { useOverlayContainer, useOverlayOpen } from "./overlay";
import "./overlay.css";
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
  // Truncating at the start is done by flipping the element to RTL so the ellipsis
  // lands on the left. That also reorders the text's own runs, which turns "14.mp4"
  // into "mp4.14", so the content is isolated in a <bdi>: it keeps its own left to
  // right order while the element still clips from the left.
  const content = truncate === "start" ? <bdi>{children}</bdi> : children;
  return (
    <span className={numeric ? "ui-value numeric" : "ui-value"} data-truncate={truncate} title={title}>
      {content}
    </span>
  );
}

interface CommitProps {
  value: string;
  onCommit: (value: string) => void;
  disabled?: boolean;
  ariaLabel?: string;
}

/**
 * Text input that commits on Enter or blur and reverts on Escape.
 *
 * `focused` is for an input that replaces the text it edits, as renaming in place does:
 * it takes the caret on mount and selects what is there, so typing replaces the old name.
 * `onDone` then says the input is finished with, whether it committed or reverted.
 */
export function TextInput({ value, onCommit, disabled, ariaLabel, numeric, focused, onDone }: CommitProps & {
  numeric?: boolean;
  focused?: boolean;
  onDone?: () => void;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      className={numeric ? "ui-input numeric" : "ui-input"}
      type={numeric ? "number" : "text"}
      value={draft}
      disabled={disabled}
      aria-label={ariaLabel}
      autoFocus={focused}
      onFocus={focused ? (e) => e.currentTarget.select() : undefined}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== value) onCommit(draft);
        onDone?.();
      }}
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
  const container = useOverlayContainer();
  const [open, setOpen] = useOverlayOpen();
  return (
    <SelectPrimitive.Root value={value} onValueChange={onChange} disabled={disabled} open={open} onOpenChange={setOpen}>
      <SelectPrimitive.Trigger className="ui-input ui-select-trigger" aria-label={ariaLabel}>
        <SelectPrimitive.Value />
        <SelectPrimitive.Icon asChild>
          <Icon name="chevronDown" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal container={container}>
        <SelectPrimitive.Content
          className="ui-overlay ui-select-content"
          data-surface="well"
          position="popper"
          sideOffset={4}
        >
          <SelectPrimitive.Viewport>
            {options.map((o) => (
              <SelectPrimitive.Item key={o.value} value={o.value} className="ui-menu-item ui-select-item">
                <SelectPrimitive.ItemText>{o.label}</SelectPrimitive.ItemText>
                <SelectPrimitive.ItemIndicator>
                  <Icon name="check" />
                </SelectPrimitive.ItemIndicator>
              </SelectPrimitive.Item>
            ))}
          </SelectPrimitive.Viewport>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
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

export interface RadioOption {
  value: string;
  label: string;
  /** One line saying what choosing this does. */
  detail?: string;
}

/**
 * Exclusive choices, all of them visible at once and each with room for a line of
 * explanation. Use it where the choice deserves to be read before it is made; use
 * Select where the options are a list of names and the panel has no room.
 *
 * The input is a real radio, so arrow keys move between the options and the group reads
 * correctly, and `Row` supplies the hover and selected fills every other choosable
 * thing in the app uses.
 */
export function RadioGroup({ value, options, onChange, name, ariaLabel }: {
  value: string;
  options: RadioOption[];
  onChange: (value: string) => void;
  /** Ties the inputs into one group, so only one can be chosen and arrows move within it. */
  name: string;
  ariaLabel: string;
}) {
  return (
    <div className="ui-radio-group" role="radiogroup" aria-label={ariaLabel}>
      {options.map((option) => (
        <Row
          key={option.value}
          element="label"
          className="ui-radio"
          marker="outline"
          selected={option.value === value}
        >
          <input
            className="ui-radio-input"
            type="radio"
            name={name}
            value={option.value}
            checked={option.value === value}
            onChange={() => onChange(option.value)}
          />
          <span className="ui-radio-text">
            <span>{option.label}</span>
            {option.detail && <span className="ui-radio-detail">{option.detail}</span>}
          </span>
        </Row>
      ))}
    </div>
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
    <SliderPrimitive.Root
      className={className ? `ui-slider ${className}` : "ui-slider"}
      min={min}
      max={max}
      step={step}
      value={[value]}
      onValueChange={(values) => values[0] !== undefined && onChange(values[0])}
    >
      <SliderPrimitive.Track className="ui-slider-track">
        <SliderPrimitive.Range className="ui-slider-range" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb className="ui-slider-thumb" aria-label={ariaLabel} />
    </SliderPrimitive.Root>
  );
}
