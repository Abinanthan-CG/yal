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
