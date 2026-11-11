// The palette's side of the application.
//
// Everything the window knows comes from the Rust side, through the four
// commands the shell exposes: `search`, `suggestions`, `open_result` and
// `hide_palette`.

import { invoke } from "@tauri-apps/api/core";

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
}

/** The rows that match a query, best first. */
export function search(query: string): Promise<ResultRow[]> {
  return invoke<ResultRow[]>("search", { query });
}

/** The rows with nothing typed: what the user opens most. */
export function suggestions(): Promise<ResultRow[]> {
  return invoke<ResultRow[]>("suggestions");
}

/** Open a row, and learn that the user opened it. */
export function openResult(id: string, query: string): Promise<void> {
  return invoke<void>("open_result", { id, query });
}

/** Hide the palette. */
export function hidePalette(): Promise<void> {
  return invoke<void>("hide_palette");
}
