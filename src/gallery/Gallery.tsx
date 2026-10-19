// The design gallery: every token, type role, primitive and the timeline painter under
// the active theme, on one page. Open it with #gallery. It is the workbench for theme
// exploration: change a theme file and every decision updates here at once.

import { useEffect, useMemo, useRef, useState } from "react";
import { sampleSnapshot } from "../preview/sampleProject";
import { applyTheme, surfaceColors, themes, useTheme, useThemeRevision, type Theme, type TypeRoleName } from "../theme";
import { draw } from "../timeline/draw";
import { contentHeight, layoutRows } from "../timeline/geometry";
import {
  Actions,
  Button,
  Checkbox,
  EmptyState,
  Field,
  Icon,
  IconButton,
  iconNames,
  Menu,
  Note,
  Popover,
  Segmented,
  Tabs,
  NumberInput,
  ProgressBar,
  Section,
  Select,
  Slider,
  TextInput,
  Timecode,
  Value,
} from "../ui";
import "./gallery.css";

function colorEntries(theme: Theme): [string, string][] {
  const out: [string, string][] = [];
  const walk = (prefix: string, node: Record<string, unknown>) => {
    for (const [key, value] of Object.entries(node)) {
      const path = prefix ? `${prefix}.${key}` : key;
      if (typeof value === "string") out.push([path, value]);
      else walk(path, value as Record<string, unknown>);
    }
  };
  walk("", theme.color as unknown as Record<string, unknown>);
  return out;
}

const SAMPLE_TEXT: Record<TypeRoleName, string> = {
  caption: "3840×2160, 24 fps",
  label: "Duration (frames)",
  body: "Import footage, audio or stills to start cutting.",
  title: "Sydney tour recap",
  heading: "Timing",
  timecode: "00:01:24:12",
  timecodeDisplay: "00:01:24:12",
  canvas: "A001_C004_stage_wide.mov",
  wordmark: "STUDIO",
  overline: "RECENT PROJECTS",
  projectName: "Postcards Rough Cut",
};

export function Gallery() {
  const theme = useTheme();
  const [checked, setChecked] = useState(true);
  const [slider, setSlider] = useState(0.4);
  const [segment, setSegment] = useState("grid");
  const [tab, setTab] = useState("inspector");

  return (
    <div className="gallery">
      <header className="gallery-header" data-surface="chrome">
        <h1>Design gallery</h1>
        <Select
          ariaLabel="Theme"
          value={theme.id}
          options={themes.map((t) => ({ value: t.id, label: t.name }))}
          onChange={(id) => {
            const next = themes.find((t) => t.id === id);
            if (next) void applyTheme(next, { remember: true });
          }}
        />
        <a className="gallery-link" href="#">Back to the editor</a>
      </header>

      <div className="gallery-grid">
        <Section title="Color">
          <div className="gallery-swatches">
            {colorEntries(theme).map(([path, value]) => (
              <div key={path} className="gallery-swatch">
                <span className="gallery-chip" style={{ background: value }} />
                <span className="gallery-token">{path}</span>
                <span className="gallery-value numeric">{value}</span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Type">
          {(Object.keys(theme.type) as TypeRoleName[]).map((role) => {
            const spec = theme.type[role];
            return (
              <div key={role} className="gallery-type">
                <span className="gallery-type-sample" data-role={role}>
                  {SAMPLE_TEXT[role]}
                </span>
                <span className="gallery-token numeric">
                  {role}: {spec.family} {spec.size}/{spec.lineHeight} {spec.weight}
                  {spec.tracking ? ` +${spec.tracking}em` : ""}
                </span>
              </div>
            );
          })}
        </Section>

        <Section title="Spacing and radius">
          {Object.entries(theme.space).map(([key, value]) => (
            <div key={key} className="gallery-space">
              <span className="gallery-token">space.{key}</span>
              <span className="gallery-bar" style={{ width: value }} />
              <span className="gallery-value numeric">{value}</span>
            </div>
          ))}
          <div className="gallery-radii">
            {Object.entries(theme.radius)
              .filter(([key]) => key !== "round")
              .map(([key, value]) => (
                <div key={key} className="gallery-radius" style={{ borderRadius: value }}>
                  <span className="gallery-token">{key}</span>
                </div>
              ))}
          </div>
        </Section>

        <Section title="Buttons and icons">
          <Actions>
            <Button variant="solid">Solid</Button>
            <Button>Quiet</Button>
            <Button variant="ghost">Ghost</Button>
            <Button aria-pressed>Pressed</Button>
            <Button disabled>Disabled</Button>
            <Button icon="plus">With icon</Button>
          </Actions>
          <div className="gallery-icons">
            {iconNames.map((name) => (
              <span key={name} className="gallery-icon" title={name}>
                <Icon name={name} />
                <span className="gallery-token">{name}</span>
              </span>
            ))}
          </div>
          <Actions>
            <IconButton icon="eye" label="Default" />
            <IconButton icon="eye" label="Small" size="small" />
            <IconButton icon="lock" label="Pressed" pressed />
            <IconButton icon="close" label="Disabled" disabled />
            <IconButton icon="play" label="Round" shape="round" />
          </Actions>
        </Section>

        <Section title="Fields">
          <Field label="Name">
            <TextInput value="A002_C011_crowd_handheld.mov" onCommit={() => {}} />
          </Field>
          <Field label="Start (frames)">
            <NumberInput value={180} onCommit={() => {}} />
          </Field>
          <Field label="Frame rate">
            <Select
              value="24/1"
              options={[
                { value: "24/1", label: "24 fps" },
                { value: "25/1", label: "25 fps" },
              ]}
              onChange={() => {}}
            />
          </Field>
          <Field label="Enabled">
            <Checkbox checked={checked} onChange={setChecked} />
          </Field>
          <Field label="Zoom">
            <Slider value={slider} min={0} max={1} step={0.01} onChange={setSlider} ariaLabel="Zoom" />
          </Field>
          <Field label="Source in">
            <Value numeric>00:00:01:00</Value>
          </Field>
          <Note>Frame rate is fixed once the timeline has clips.</Note>
        </Section>

        <Section title="Menus and overlays">
          <Actions>
            <Menu
              trigger={<Button icon="file">File</Button>}
              entries={[
                { label: "New project", icon: "plus", shortcut: "⌘N", onSelect: () => {} },
                { label: "Open…", shortcut: "⌘O", onSelect: () => {} },
                { type: "separator" },
                { type: "label", label: "Recent" },
                { label: "Sydney tour recap", onSelect: () => {} },
                { type: "separator" },
                { label: "Remove media", icon: "trash", tone: "danger", onSelect: () => {} },
              ]}
            />
            <Popover label="Snapping" trigger={<Button icon="magnet">Popover</Button>}>
              <Field label="Snap distance">
                <NumberInput value={8} onCommit={() => {}} />
              </Field>
            </Popover>
            <IconButton icon="scissors" label="Split at playhead" shortcut="S" />
          </Actions>
          <Actions>
            <Segmented
              label="View"
              value={segment}
              onChange={setSegment}
              options={[
                { value: "grid", label: "Grid view", icon: "grid" },
                { value: "list", label: "List view", icon: "list" },
              ]}
            />
            <Segmented
              label="Edit mode"
              value={segment}
              onChange={setSegment}
              options={[
                { value: "grid", label: "Overwrite" },
                { value: "list", label: "Insert" },
              ]}
            />
          </Actions>
          <Tabs
            label="Inspector tabs"
            value={tab}
            onChange={setTab}
            tabs={[
              { value: "inspector", label: "Inspector", content: <Note>Clip properties go here.</Note> },
              { value: "effects", label: "Effects", content: <Note>Effects go here.</Note> },
              { value: "color", label: "Color", content: <Note>Color goes here.</Note> },
            ]}
          />
        </Section>

        <Section title="Readouts">
          <div className="gallery-readouts">
            <Timecode frame={2028} rate={{ num: 24, den: 1 }} size="display" />
            <Timecode frame={2028} rate={{ num: 24, den: 1 }} />
            <Timecode frame={8640} rate={{ num: 24, den: 1 }} size="body" tone="muted" />
            <ProgressBar fraction={0.62} />
          </div>
          <div className="gallery-empty">
            <EmptyState action={<Button variant="solid">Import media</Button>}>
              Import footage, audio or stills to start cutting.
            </EmptyState>
          </div>
        </Section>
      </div>

      <Section title="Timeline">
        <TimelineSample />
      </Section>
    </div>
  );
}

/** The real timeline painter on the sample project, so canvas styling is reviewable. */
function TimelineSample() {
  const theme = useTheme();
  const revision = useThemeRevision();
  const ref = useRef<HTMLCanvasElement>(null);
  const project = useMemo(() => sampleSnapshot().project, []);
  const sequence = project.sequences[0]!;
  const rows = layoutRows(sequence, theme.timeline);
  const height = contentHeight(rows, theme.timeline);
  const selected = sequence.tracks[0]!.clips[1]!;

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const width = canvas.parentElement?.clientWidth ?? 0;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(ctx, {
      theme,
      colors: surfaceColors(theme, "stage"),
      project,
      sequence,
      rows,
      view: { zoom: 3, scrollX: 0 },
      width,
      height,
      playhead: 132,
      selection: new Set(sequence.tracks.flatMap((t) => t.clips.filter((c) => c.link === selected.link).map((c) => c.id))),
      offline: new Set(),
      lifted: new Set(),
      ghosts: [],
    });
  }, [theme, revision, project, rows, height, selected]);

  return (
    <div className="gallery-timeline">
      <canvas ref={ref} />
    </div>
  );
}
