import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import { Tooltip } from "./Tooltip";
import "./button.css";

export type ButtonVariant = "default" | "primary" | "ghost";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  icon?: IconName;
  children?: ReactNode;
}

/** Text button. Variants are styled in button.css from theme tokens. */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "default", icon, children, type = "button", className, ...rest },
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
  /** "round" is the large circular transport button. */
  shape?: "square" | "round";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { icon, label, shortcut, pressed, shape = "square", type = "button", className, ...rest },
  ref,
) {
  return (
    <Tooltip label={label} shortcut={shortcut}>
      <button
        ref={ref}
        type={type}
        className={className ? `ui-icon-button ${className}` : "ui-icon-button"}
        data-shape={shape}
        aria-label={label}
        aria-pressed={pressed}
        {...rest}
      >
        <Icon name={icon} />
      </button>
    </Tooltip>
  );
});
