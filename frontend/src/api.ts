// The palette's side of the application.
//
// Everything the window knows comes from the Rust side, through the commands
// the shell exposes: `search`, `suggestions`, `open_result`,
// `run_custom_command` and `hide_palette`.

import { invoke } from "@tauri-apps/api/core";

/** One value a command asks for before it runs. */
export interface CommandArgument {
  /** The name shown beside the field. */
  name: string;
  /** Whether the field may be left empty. */
  isOptional: boolean;
}

/** One row in the results list. */
export interface ResultRow {
  /** The key the launcher opens the row by. */
  id: string;
  /** The name the row is shown under. */
  name: string;
  /** The line under the name. */
  detail: string;
  /** The section the row belongs to. */
  kind: string;
  /** What the launcher opens. */
  target: string;
  /** Whether the launcher has to ask before it does it. */
  needsConfirm: boolean;
  /** The values the command asks for first. */
  arguments?: CommandArgument[];
  /** Whether the row is a folder. Only the file rows say. */
  isDir?: boolean;
}

/** A key press, as the window sees it. */
export interface KeyStroke {
  /** The name the browser gives the key. */
  key: string;
  /** Whether Ctrl was held. */
  ctrl: boolean;
  /** Whether Alt was held. */
  alt: boolean;
  /** Whether Shift was held. */
  shift: boolean;
  /** Whether the Windows key was held. */
  win: boolean;
}

/** What a key press did. */
export interface KeyOutcome {
  /** The action the palette decided on, if any. */
  action: string | null;
  /** Whether the palette closed. */
  closed: boolean;
  /** Whether the palette went back a screen. */
  wentBack: boolean;
  /** Whether the palette collapsed into its slim bar. */
  collapsed: boolean;
}

/** One entry of the actions menu. */
export interface MenuItem {
  /** The name the entry is shown under. */
  label: string;
  /** The chord, when the action has one. */
  shortcut: string | null;
  /** Whether the entry is the highlighted one. */
  selected: boolean;
}

/** What the palette looks like right now. */
export interface PaletteView {
  /** The screen on top. */
  screen: string;
  /** How many screens are stacked. */
  depth: number;
  /** The text in the search field. */
  query: string;
  /** Whether the list is showing. */
  expanded: boolean;
  /** What the slim bar is showing. */
  compactBar: string;
  /** The kind of file the file search is narrowed to. */
  fileFilter: string;
  /** The actions of the menu, when it is open. */
  menu: MenuItem[];
  /** Whether the menu is open. */
  menuOpen: boolean;
  /** How many fields the selected row takes. */
  fields: number;
  /** Which field has the focus. */
  focused: number;
  /** How the palette is painted. */
  appearance: string;
  /** Whether the palette moves when it opens and closes. */
  animations: boolean;
  /** The factor the interface is drawn at. */
  scale: number;
}

/** The rows that match a query, best first. */
export function search(query: string): Promise<ResultRow[]> {
  return invoke<ResultRow[]>("search", { query });
}

/** The rows with nothing typed: what the user opens most. */
export function suggestions(): Promise<ResultRow[]> {
  return invoke<ResultRow[]>("suggestions");
}

/**
 * Open a row, and learn that the user opened it.
 *
 * A row that has to be confirmed is not opened until `confirmed` says so.
 */
export function openResult(id: string, query: string, confirmed = false): Promise<void> {
  return invoke<void>("open_result", { id, query, confirmed });
}

/**
 * Run one of the user's own commands.
 *
 * The values are handed over in the order the command asks for them, and they
 * are never written into the command text.
 */
export function runCustomCommand(id: string, values: string[]): Promise<string> {
  return invoke<string>("run_custom_command", { id, values });
}

/** Hide the palette. */
export function hidePalette(): Promise<void> {
  return invoke<void>("hide_palette");
}

/**
 * Press a key.
 *
 * The palette decides what the key means; the window is told what to do
 * about it. The window never decides on its own.
 */
export function paletteKey(stroke: KeyStroke, query: string): Promise<KeyOutcome> {
  return invoke<KeyOutcome>("palette_key", { stroke, query });
}

/** What the palette looks like right now. */
export function paletteView(): Promise<PaletteView> {
  return invoke<PaletteView>("palette_view");
}

/** Type in the actions menu's search field. */
export function paletteMenuQuery(query: string): Promise<void> {
  return invoke<void>("palette_menu_query", { query });
}

/** Open a screen by name. */
export function paletteOpenScreen(screen: string): Promise<void> {
  return invoke<void>("palette_open_screen", { screen });
}

/** Go back one screen. */
export function paletteBack(): Promise<boolean> {
  return invoke<boolean>("palette_back");
}

/** Go back to the root search from any screen. */
export function paletteBackToRoot(): Promise<void> {
  return invoke<void>("palette_back_to_root");
}

/** The rows of the file-search screen. */
export function fileRows(query: string): Promise<ResultRow[]> {
  return invoke<ResultRow[]>("file_rows", { query });
}

/** Open a file or a folder. */
export function openFile(path: string): Promise<void> {
  return invoke<void>("open_file", { path });
}

/** Show a file in the file manager, with it selected. */
export function revealFile(path: string): Promise<void> {
  return invoke<void>("reveal_file", { path });
}

/** One row of the Settings pane. */
export interface SettingRow {
  /** The key the setting is stored under. */
  key: string;
  /** The name the setting is shown under. */
  label: string;
  /** The value the setting has now. */
  value: string;
  /** The values it can take. */
  options: string[];
}

/** The Settings pane's rows. */
export function paletteSettings(): Promise<SettingRow[]> {
  return invoke<SettingRow[]>("palette_settings");
}

/** Move a setting to its next value, and write it down. */
export function paletteAdvanceSetting(key: string): Promise<string> {
  return invoke<string>("palette_advance_setting", { key });
}

/** Run one of a file row's actions. */
export function fileAction(action: string, path: string): Promise<string> {
  return invoke<string>("file_action", { action, path });
}
