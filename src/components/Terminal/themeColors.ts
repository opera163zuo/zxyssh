/**
 * The one palette the application ships with. It mirrors the `--tokyo-*` CSS
 * variables in `styles.css`; components that paint inline styles (the completion
 * popup, xterm's theme) need the same values as TypeScript literals.
 */
export const themeColors = {
  bg: '#ffffff',
  bgDark: '#f7f7f5',
  bgHl: '#e7e7e3',
  fg: '#171717',
  fgDark: '#73736e',
  accent: '#6d4aff',
  onAccent: '#ffffff',
  red: '#b52f3e',
  green: '#18714a',
  yellow: '#805200',
  magenta: '#743796',
  cyan: '#087477',
  orange: '#974117',
};

export type ThemeColors = typeof themeColors;
export type ThemeColorKey = keyof ThemeColors;

/** xterm.js palette, derived from the same colours. */
export const xtermTheme = {
  background: themeColors.bg,
  foreground: themeColors.fg,
  cursor: themeColors.accent,
  cursorAccent: themeColors.bg,
  selectionBackground: themeColors.bgHl,
  black: themeColors.bgHl,
  red: themeColors.red,
  green: themeColors.green,
  yellow: themeColors.yellow,
  blue: themeColors.accent,
  magenta: themeColors.magenta,
  cyan: themeColors.cyan,
  white: themeColors.fg,
  brightBlack: themeColors.fgDark,
  brightRed: themeColors.red,
  brightGreen: themeColors.green,
  brightYellow: themeColors.yellow,
  brightBlue: themeColors.accent,
  brightMagenta: themeColors.magenta,
  brightCyan: themeColors.cyan,
  brightWhite: themeColors.fg,
};
