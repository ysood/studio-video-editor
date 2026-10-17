import type { Rational } from "../bindings/Rational";
import { timecode } from "../format";
import "./timecode.css";

/**
 * A timecode readout. "toolbar" and "display" map to the `timecode` and
 * `timecodeDisplay` type roles; "muted" is a secondary readout such as a duration.
 */
export function Timecode({ frame, rate, size = "toolbar", tone = "accent", label }: {
  frame: number;
  rate: Rational;
  size?: "toolbar" | "display" | "body";
  tone?: "accent" | "muted";
  label?: string;
}) {
  return (
    <span className="ui-timecode numeric" data-size={size} data-tone={tone} aria-label={label}>
      {timecode(frame, rate)}
    </span>
  );
}
