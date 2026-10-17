import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import { Tooltip } from "./Tooltip";
import "./button.css";

/**
 * Button hierarchy, the same on every surface:
 * - solid: the one primary action in a region (Export, Import media, a dialog's confirm)
 * - quiet: the default for actions
 * - ghost: low-emphasis actions inside dense toolbars and lists
 */
export type ButtonVariant = "solid" | "quiet" | "ghost";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  icon?: IconName;
  children?: ReactNode;
}

/** Text button. Variants are styled in button.css from theme tokens. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "quiet", icon, children, type = "button", className, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={className ? `ui-button ${className}` : "ui-button"}
      data-variant={variant}
      {...rest}
    >
      {icon && <Icon name={icon} />}
      {children}
    </button>
  );
});

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  icon: IconName;
  /** Accessible name, also shown as the tooltip. */
  label: string;
  /** Keyboard shortcut shown in the tooltip, e.g. "Space". */
  shortcut?: string;
  pressed?: boolean;
  /** "round" is the large circular transport button; it uses the solid style. */
  shape?: "square" | "round";
  /** "small" for dense rows such as track headers. */
  size?: "default" | "small";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { icon, label, shortcut, pressed, shape = "square", size = "default", type = "button", className, ...rest },
  ref,
) {
  return (
    <Tooltip label={label} shortcut={shortcut}>
      <button
        ref={ref}
        type={type}
        className={className ? `ui-icon-button ${className}` : "ui-icon-button"}
        data-shape={shape}
        data-size={size}
        aria-label={label}
        aria-pressed={pressed}
        {...rest}
      >
        <Icon name={icon} />
      </button>
    </Tooltip>
  );
});
