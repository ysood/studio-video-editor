import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "../lib/cn";
import { Icon, type IconName } from "./Icon";
import { Tooltip } from "./Tooltip";

/**
 * Button hierarchy, the same on every surface:
 * - solid: the one primary action in a region (Export, Import media, a dialog's confirm)
 * - quiet: the default for actions
 * - ghost: low-emphasis actions inside dense toolbars and lists
 *
 * There is no fourth. A region that seems to need two solid buttons has two things
 * claiming to be the main one, which is a layout problem rather than a variant problem.
 */
export type ButtonVariant = "solid" | "quiet" | "ghost";

/**
 * Everything every control in Studio shares: one height, one radius, one focus ring,
 * one transition, one disabled treatment. A control that opts out of a line here is the
 * reason an interface stops looking like one product, so new controls spread this list
 * rather than restating parts of it.
 *
 * `cursor-default` is deliberate: this is a desktop application, and native desktop
 * controls do not put a hand cursor under a button.
 */
export const controlBase = cn(
  "inline-flex shrink-0 items-center justify-center gap-3 whitespace-nowrap",
  "cursor-default select-none outline-none",
  "transition-colors duration-quick ease-standard",
  "focus-visible:focus-ring",
  "disabled:pointer-events-none disabled:opacity-disabled",
  "[&_svg]:pointer-events-none [&_svg]:shrink-0",
);

const buttonVariants = cva(cn(controlBase, "rounded-button"), {
  variants: {
    variant: {
      solid: "bg-accent text-fg-on-accent hover:bg-accent-hover",
      quiet: "bg-raised text-fg hover:state-hover",
      ghost: "bg-transparent text-fg-muted hover:text-fg hover:state-hover",
    },
    size: {
      // A button is wider than it is tall by about three spacing steps at each end;
      // text that sits tight against a corner reads as cramped however correct the
      // height is.
      default: "h-control px-6 text-body",
      small: "h-control-small px-5 text-label",
      large: "h-control-large px-7 text-body",
    },
  },
  defaultVariants: { variant: "quiet", size: "default" },
});

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  icon?: IconName;
  children?: ReactNode;
  /** Render as the single child element instead of a <button>, keeping the styling. */
  asChild?: boolean;
}

/** Text button. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant, size, icon, children, type = "button", className, asChild, ...rest },
  ref,
) {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp
      ref={ref}
      type={asChild ? undefined : type}
      className={cn(
        buttonVariants({ variant, size }),
        // A toggle that is on reads the same everywhere: accent text on an accent wash.
        "aria-pressed:bg-accent-wash aria-pressed:text-accent",
        className,
      )}
      {...rest}
    >
      {icon && <Icon name={icon} />}
      {children}
    </Comp>
  );
});

/**
 * A button with no look of its own: the browser's chrome stripped and the focus ring
 * kept, and nothing else. It is for a control whose content is a block of layout rather
 * than a label, such as a whole row that is one target, where Button's height and
 * padding would be in the way. Anything that should read as a button is a Button.
 */
export const PlainButton = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement>>(
  function PlainButton({ children, type = "button", className, ...rest }, ref) {
    return (
      <button
        ref={ref}
        type={type}
        className={cn(
          "cursor-default rounded-inherit border-0 bg-transparent p-0 text-start outline-none",
          "focus-visible:focus-ring disabled:opacity-disabled",
          className,
        )}
        {...rest}
      >
        {children}
      </button>
    );
  },
);

const iconButtonVariants = cva(cn(controlBase, "p-0"), {
  variants: {
    shape: {
      square: "rounded-button bg-transparent text-fg-muted hover:text-fg hover:state-hover",
      // The transport's play button: the one place a round control is right, because it
      // is the only control in the product that is a target before it is a label.
      round: "rounded-round bg-accent text-fg-on-accent hover:bg-accent-hover",
    },
    size: {
      default: "size-control",
      small: "size-control-small",
    },
  },
  compoundVariants: [{ shape: "round", size: "default", class: "size-play-button" }],
  defaultVariants: { shape: "square", size: "default" },
});

export interface IconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children">,
    VariantProps<typeof iconButtonVariants> {
  icon: IconName;
  /** Accessible name, also shown as the tooltip. An icon must always explain itself. */
  label: string;
  /** Keyboard shortcut shown in the tooltip, e.g. "Space". */
  shortcut?: string;
  pressed?: boolean;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { icon, label, shortcut, pressed, shape, size, type = "button", className, ...rest },
  ref,
) {
  return (
    <Tooltip label={label} shortcut={shortcut}>
      <button
        ref={ref}
        type={type}
        className={cn(
          iconButtonVariants({ shape, size }),
          "aria-pressed:bg-accent-wash aria-pressed:text-accent",
          className,
        )}
        aria-label={label}
        aria-pressed={pressed}
        {...rest}
      >
        <Icon name={icon} />
      </button>
    </Tooltip>
  );
});
