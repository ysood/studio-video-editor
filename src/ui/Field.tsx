import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import * as RadioGroupPrimitive from "@radix-ui/react-radio-group";
import * as SelectPrimitive from "@radix-ui/react-select";
import * as SliderPrimitive from "@radix-ui/react-slider";
import { useEffect, useState, type ReactNode } from "react";
import { cn } from "../lib/cn";
import { controlBase, IconButton } from "./Button";
import { Icon } from "./Icon";
import { Row } from "./List";
import { useOverlayContainer, useOverlayOpen } from "./overlay";
import { overlayItem, overlayMenuPanel } from "./overlayStyles";

/**
 * The one input look, shared by text fields and by anything that has to sit beside one
 * and read as its equal, such as a select trigger. A sunk fill, a hairline that firms
 * up under the pointer, and the product's single focus ring.
 */
export const inputBase = cn(
  controlBase,
  // The outline is the whole of what marks a field, since its fill is the panel's
  // own black, so it takes the firmer of the two hairlines.
  "h-control w-full justify-between rounded-control border border-edge-strong bg-field px-4",
  // The value is secondary text at rest and primary once the field has focus. A panel
  // of fields is mostly values, and with every one of them at full white the panel had
  // no quiet part; the field you are in is now the one that stands out.
  "text-label text-fg-muted focus:text-fg",
  "hover:border-fg-faint",
  "disabled:opacity-disabled data-[disabled]:opacity-disabled",
);

/** A labelled row: label on the left, control on the right. */
export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="grid-field grid min-h-control items-center gap-4 text-label">
      <span className="truncate text-fg-muted">{label}</span>
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
    <span
      className={cn("truncate", numeric && "numeric", truncate === "start" && "text-left")}
      dir={truncate === "start" ? "rtl" : undefined}
      title={title}
    >
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
export function TextInput({ value, onCommit, disabled, ariaLabel, numeric, focused, onDone, className }: CommitProps & {
  numeric?: boolean;
  focused?: boolean;
  onDone?: () => void;
  className?: string;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      className={cn(inputBase, "cursor-text select-text", numeric && "numeric", className)}
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

/**
 * A filter field: unlike TextInput it reports every keystroke, because a filter that
 * waits for Enter is not filtering as you type. Escape empties it, and a clear button
 * shows while there is text so the mouse has the same way out.
 */
export function SearchInput({ value, onChange, ariaLabel, placeholder, className }: {
  value: string;
  onChange: (value: string) => void;
  ariaLabel: string;
  placeholder?: string;
  className?: string;
}) {
  return (
    // The wrapper is the field and the input inside it is bare. The magnifier used to be
    // laid over a padded input, which cost the text a gutter far wider than the glyph,
    // and the focus ring lifts whatever it is on above its neighbours, so a focused
    // input painted its own fill over the icon. As siblings in one row the text starts
    // where the icon ends and the ring goes round all of it.
    <div
      className={cn(
        "flex h-control min-w-0 flex-1 items-center gap-3 rounded-control border border-edge-strong bg-field ps-4",
        "text-fg-faint transition-colors duration-quick ease-standard",
        "hover:border-fg-faint has-focus-visible:focus-ring",
        value === "" ? "pe-4" : "pe-1",
        className,
      )}
    >
      <Icon name="search" className="pointer-events-none shrink-0" />
      <input
        className="h-full min-w-0 flex-1 cursor-text bg-transparent text-label text-fg-muted outline-none select-text focus:text-fg"
        type="text"
        role="searchbox"
        value={value}
        placeholder={placeholder}
        aria-label={ariaLabel}
        autoComplete="off"
        spellCheck={false}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          // Only swallow Escape while there is something to clear, so an empty field
          // still lets Escape reach whatever else is listening for it.
          if (e.key === "Escape" && value !== "") {
            e.preventDefault();
            e.stopPropagation();
            onChange("");
          }
        }}
      />
      {value !== "" && (
        <IconButton
          size="small"
          icon="close"
          label="Clear search"
          onClick={() => onChange("")}
        />
      )}
    </div>
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
      <SelectPrimitive.Trigger className={cn(inputBase, "gap-4")} aria-label={ariaLabel}>
        <SelectPrimitive.Value />
        <SelectPrimitive.Icon asChild>
          <Icon name="chevronDown" className="text-fg-muted" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal container={container}>
        <SelectPrimitive.Content
          className={cn(overlayMenuPanel, "w-(--radix-select-trigger-width)")}
          data-surface="well"
          position="popper"
          sideOffset={4}
        >
          <SelectPrimitive.Viewport>
            {options.map((o) => (
              <SelectPrimitive.Item key={o.value} value={o.value} className={overlayItem}>
                <SelectPrimitive.ItemText>{o.label}</SelectPrimitive.ItemText>
                <SelectPrimitive.ItemIndicator className="ms-auto ps-6">
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

/**
 * A real box with a drawn check, not the browser's widget tinted with `accent-color`.
 * The native control cannot take the product's radius, border or focus ring, which is
 * most of why it used to read as something pasted in from another application.
 */
export function Checkbox({ checked, onChange, disabled, ariaLabel }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  ariaLabel?: string;
}) {
  return (
    <CheckboxPrimitive.Root
      className={cn(
        controlBase,
        "size-icon justify-self-start rounded-control border border-edge-strong bg-field",
        "data-[state=checked]:border-accent data-[state=checked]:bg-accent data-[state=checked]:text-fg-on-accent",
        "[&_svg]:size-6",
      )}
      checked={checked}
      disabled={disabled}
      aria-label={ariaLabel}
      onCheckedChange={(next) => onChange(next === true)}
    >
      <CheckboxPrimitive.Indicator asChild>
        <Icon name="check" />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
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
 * `Row` supplies the hover and selected fills that every other choosable thing in the
 * app uses, so a chosen export preset and a chosen clip are marked the same way.
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
    <RadioGroupPrimitive.Root
      className="grid gap-2"
      value={value}
      name={name}
      aria-label={ariaLabel}
      onValueChange={onChange}
    >
      {options.map((option) => (
        // The whole card is the target, so the Item *is* the row rather than a dot
        // inside it: clicking the explanation chooses the option, as it must.
        <RadioGroupPrimitive.Item key={option.value} value={option.value} asChild>
          <Row
            element="div"
            // Not `controlBase`: that describes a control one line tall, and its
            // `whitespace-nowrap` would stop an option's explanation wrapping inside
            // the card. A card takes the parts of it that are about being a target.
            className={cn(
              "grid-lead grid cursor-default gap-5 rounded-control p-5 text-start",
              "outline-none focus-visible:focus-ring",
            )}
            marker="outline"
            selected={option.value === value}
          >
            <span
              className={cn(
                // The dot sits on the first line of the label rather than centred
                // against a block that may be one line or two.
                "mt-1 grid size-icon place-items-center self-start rounded-round",
                "border border-edge-strong bg-field",
                option.value === value && "border-accent",
              )}
            >
              <RadioGroupPrimitive.Indicator className="size-dot rounded-round bg-accent" />
            </span>
            {/* min-w-0, or a grid child keeps its content's intrinsic width and a long
                explanation runs out past the card instead of wrapping inside it. */}
            <span className="grid min-w-0 gap-1">
              <span>{option.label}</span>
              {option.detail && <span className="text-label text-fg-muted">{option.detail}</span>}
            </span>
          </Row>
        </RadioGroupPrimitive.Item>
      ))}
    </RadioGroupPrimitive.Root>
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
      className={cn("relative flex h-control w-full touch-none items-center select-none", className)}
      min={min}
      max={max}
      step={step}
      value={[value]}
      onValueChange={(values) => values[0] !== undefined && onChange(values[0])}
    >
      <SliderPrimitive.Track className="relative h-slider-track w-full grow overflow-hidden rounded-round bg-raised">
        {/* The travelled part is the accent, because a slider is a value being set and
            the accent is the colour of committing one. */}
        <SliderPrimitive.Range className="absolute h-full bg-accent" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        className={cn(
          controlBase,
          // White, not the accent: the travelled range already carries the accent, and a
          // handle has to be found against the rail rather than agree with it.
          "block size-slider-thumb rounded-round bg-fg shadow-popover",
        )}
        aria-label={ariaLabel}
      />
    </SliderPrimitive.Root>
  );
}
