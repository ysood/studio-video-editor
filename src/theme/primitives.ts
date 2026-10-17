// Raw values. Themes pick from these; components never import this file.

export const graphite = {
  50: "#e4e4e7",
  300: "#9b9ca3",
  500: "#6c6e75",
  700: "#3a3c42",
  750: "#303237",
  800: "#2d2f33",
  850: "#26282c",
  900: "#1f2023",
  950: "#18191b",
  1000: "#000000",
} as const;

export const steel = { 500: "#4d79a8", 300: "#6e97c4" } as const;
export const sea = { 500: "#3d8a77", 300: "#5fae99" } as const;
export const amber = { 400: "#f6c565", 500: "#f0b545", 950: "#1d1606" } as const;
export const coral = { 500: "#e0645a" } as const;

export const systemSans =
  '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", "Segoe UI", sans-serif';
export const systemMono = 'ui-monospace, "SF Mono", Menlo, Consolas, monospace';
