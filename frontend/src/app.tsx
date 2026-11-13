// The palette.
//
// One field at the top, the results under it. The window is shown and hidden
// rather than opened and closed, so the field is focused every time the
// palette appears.
//
// The window does not decide what a key means. It sends the key to the Rust
// side, which owns the palette's state - the screens, the stack, the tab
// cycle, the escape behaviour - and is told what to do about it. A decision
// that cannot be tested is not a decision, and the decisions are all in
// `tc-palette`.
//
// A row of the user's own command may ask for values first, and may have to
// be confirmed before it runs; both happen inside the palette.

import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import {
  fileRows,
  hidePalette,
  openResult,
  paletteKey,
  paletteMenuQuery,
  paletteOpenScreen,
  paletteView,
  revealFile,
  runCustomCommand,
  search,
  suggestions,
  type KeyStroke,
  type PaletteView,
  type ResultRow,
} from "./api";

/** How long the typing rests before the launcher is asked. */
const SETTLE_MS = 60;

/** What the palette is showing. */
type Mode = "search" | "fields" | "confirm";

/** Whether a row is one of the user's own commands. */
function isCustomCommand(row: ResultRow): boolean {
  return row.id.startsWith("custom-command:");
}

/** Whether a row is a file or a folder. */
function isFile(row: ResultRow): boolean {
  return row.id.startsWith("file:");
}

/** The key press, as the Rust side wants it. */
function strokeOf(event: KeyboardEvent): KeyStroke {
  return {
    key: event.key,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    win: event.metaKey,
  };
}

export function App() {
  const [query, setQuery] = createSignal("");
  const [rows, setRows] = createSignal<ResultRow[]>([]);
  const [selected, setSelected] = createSignal(0);
  const [failed, setFailed] = createSignal<string | null>(null);
  const [confirming, setConfirming] = createSignal<string | null>(null);
  const [mode, setMode] = createSignal<Mode>("search");
  const [pending, setPending] = createSignal<ResultRow | null>(null);
  const [values, setValues] = createSignal<string[]>([]);
  const [field, setField] = createSignal(0);
  const [note, setNote] = createSignal<string | null>(null);
  const [view, setView] = createSignal<PaletteView | null>(null);
  let input: HTMLInputElement | undefined;
  let timer: number | undefined;

  // The screen on top, as the Rust side says it is.
  const screen = (): string => view()?.screen ?? "Launcher";

  // The rows of the screen on top. The file search has its own rows; every
  // other screen has the launcher's.
  const ask = async (text: string): Promise<void> => {
    try {
      const state = await paletteView();
      setView(state);
      let found: ResultRow[];
      if (state.screen === "Search Files") {
        found = await fileRows(text);
      } else {
        found = text.trim() === "" ? await suggestions() : await search(text);
      }
      setRows(found);
      setSelected(0);
      setFailed(null);
      setConfirming(null);
      setMode("search");
    } catch (error) {
      setRows([]);
      setFailed(String(error));
    }
  };

  // Every change of the field restarts the wait, so a fast typist asks once.
  createEffect(() => {
    const text = query();
    window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      void ask(text);
    }, SETTLE_MS);
  });

  onMount(() => {
    input?.focus();
  });

  onCleanup(() => {
    window.clearTimeout(timer);
  });

  const move = (delta: number): void => {
    const count = rows().length;
    if (count === 0) {
      return;
    }
    setSelected((current) => (current + delta + count) % count);
  };

  // Runs one of the user's own commands.
  const run = async (row: ResultRow, given: string[]): Promise<void> => {
    setNote(null);
    try {
      const message = await runCustomCommand(row.id, given);
      setNote(message);
      setMode("search");
      setPending(null);
    } catch (error) {
      setFailed(String(error));
      setMode("search");
      setPending(null);
    }
  };

  // A row that has to be confirmed is asked for first: the first Enter only
  // asks, and the second one does it.
  const activate = async (row: ResultRow): Promise<void> => {
    setFailed(null);
    setNote(null);
    if (isCustomCommand(row)) {
      const wanted = row.arguments ?? [];
      if (wanted.length > 0) {
        // The command asks for values first, so the fields come up and the
        // command stays selected while they are filled in.
        setPending(row);
        setValues(wanted.map(() => ""));
        setField(0);
        setMode("fields");
        return;
      }
      if (row.needsConfirm && confirming() !== row.id) {
        setPending(row);
        setConfirming(row.id);
        setMode("confirm");
        return;
      }
      setConfirming(null);
      setPending(null);
      await run(row, []);
      return;
    }
    if (row.needsConfirm && confirming() !== row.id) {
      setConfirming(row.id);
      return;
    }
    setConfirming(null);
    try {
      await openResult(row.id, query(), row.needsConfirm);
    } catch (error) {
      setFailed(String(error));
    }
  };

  const openSelected = async (): Promise<void> => {
    const row = rows()[selected()];
    if (!row) {
      return;
    }
    await activate(row);
  };

  // The second action of a row: show it in the file manager.
  const revealSelected = async (): Promise<void> => {
    const row = rows()[selected()];
    if (!row) {
      return;
    }
    try {
      await revealFile(row.target);
    } catch (error) {
      setFailed(String(error));
    }
  };

  // The first required value that is still empty.
  const firstMissing = (): number => {
    const row = pending();
    const wanted = row?.arguments ?? [];
    const given = values();
    const index = wanted.findIndex(
      (argument, position) => !argument.isOptional && (given[position] ?? "").trim() === "",
    );
    return index;
  };

  const submitFields = async (): Promise<void> => {
    const row = pending();
    if (!row) {
      return;
    }
    const missing = firstMissing();
    if (missing >= 0) {
      // The first missing field takes the focus.
      setField(missing);
      return;
    }
    await run(row, values());
  };

  // What the Rust side says the key means.
  const act = async (action: string): Promise<void> => {
    switch (action) {
      case "MainAction":
        await openSelected();
        break;
      case "SecondAction":
        await revealSelected();
        break;
      case "CloseWindow":
        await hidePalette();
        break;
      default:
        // Everything else changes the palette's own state, so the rows and
        // the view are asked for again.
        await ask(query());
        break;
    }
  };

  const onKeyDown = async (event: KeyboardEvent): Promise<void> => {
    if (mode() === "fields") {
      switch (event.key) {
        case "Tab":
          event.preventDefault();
          setField((current) => (current + (event.shiftKey ? -1 : 1) + values().length) % values().length);
          break;
        case "Enter":
          event.preventDefault();
          void submitFields();
          break;
        case "Escape":
          event.preventDefault();
          setMode("search");
          setPending(null);
          break;
        default:
          break;
      }
      return;
    }
    if (mode() === "confirm") {
      switch (event.key) {
        case "Enter": {
          event.preventDefault();
          const row = pending();
          if (row) {
            setConfirming(null);
            setPending(null);
            setMode("search");
            if (isCustomCommand(row)) {
              await run(row, []);
            } else {
              try {
                await openResult(row.id, query(), true);
              } catch (error) {
                setFailed(String(error));
              }
            }
          }
          break;
        }
        case "Escape":
          event.preventDefault();
          setMode("search");
          setConfirming(null);
          setPending(null);
          break;
        default:
          break;
      }
      return;
    }

    // The menu takes the keys while it is open.
    if (view()?.menuOpen) {
      if (event.key === "Escape") {
        event.preventDefault();
        const outcome = await paletteKey(strokeOf(event), query());
        await paletteMenuQuery("");
        await act(outcome.action ?? "");
        return;
      }
    }

    const outcome = await paletteKey(strokeOf(event), query());
    const action = outcome.action;
    if (!action) {
      // The key belongs to the text field.
      return;
    }
    event.preventDefault();
    if (action === "MoveSelection") {
      // The direction is carried by the selection itself; the Rust side has
      // already moved the compact bar, and the list follows the arrow keys.
      move(event.key === "ArrowUp" || (event.altKey && event.key === "p") ? -1 : 1);
      return;
    }
    if (action === "Escape" && outcome.closed) {
      await hidePalette();
      return;
    }
    await act(action);
  };

  // The menu's own search field takes the typing while the menu is open.
  const onInput = (event: InputEvent & { currentTarget: HTMLInputElement }): void => {
    setQuery(event.currentTarget.value);
    if (view()?.menuOpen) {
      void paletteMenuQuery(event.currentTarget.value);
    }
  };

  const openFileSearch = async (): Promise<void> => {
    await paletteOpenScreen("Search Files");
    await ask(query());
  };

  return (
    <main
      class="palette"
      classList={{
        "palette--dark": view()?.appearance === "Dark",
        "palette--light": view()?.appearance === "Light",
      }}
      style={{ "--yal-scale": String(view()?.scale ?? 1) }}
    >
      <header class="palette__field">
        <input
          ref={input}
          type="text"
          spellcheck={false}
          autocomplete="off"
          autocapitalize="off"
          placeholder="Search YAL"
          aria-label="Search"
          value={query()}
          onInput={onInput}
          onKeyDown={onKeyDown}
        />
      </header>
      <Show when={mode() === "fields"}>
        <section class="palette__fields">
          <For each={pending()?.arguments ?? []}>
            {(argument, index) => (
              <label
                class="palette__argument"
                classList={{ "palette__argument--focused": index() === field() }}
              >
                <span class="palette__argument-name">{argument.name}</span>
                <input
                  type="text"
                  spellcheck={false}
                  autocomplete="off"
                  aria-label={argument.name}
                  placeholder={argument.isOptional ? "optional" : "required"}
                  value={values()[index()]}
                  onFocus={() => setField(index())}
                  onInput={(event) =>
                    setValues((current) => {
                      const next = [...current];
                      next[index()] = event.currentTarget.value;
                      return next;
                    })
                  }
                  onKeyDown={onKeyDown}
                />
              </label>
            )}
          </For>
        </section>
      </Show>
      <Show when={mode() === "confirm"}>
        <section class="palette__confirm">
          <p class="palette__confirm-name">{pending()?.name}</p>
          <pre class="palette__confirm-text">{pending()?.detail}</pre>
        </section>
      </Show>
      <Show when={view()?.menuOpen}>
        <section class="palette__menu" aria-label="Actions">
          <ul class="palette__menu-list">
            <For each={view()?.menu ?? []}>
              {(item) => (
                <li
                  class="palette__menu-item"
                  classList={{ "palette__menu-item--selected": item.selected }}
                >
                  <span class="palette__menu-label">{item.label}</span>
                  <Show when={item.shortcut}>
                    <span class="palette__menu-shortcut">{item.shortcut}</span>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </section>
      </Show>
      <Show when={screen() === "Search Files"}>
        <div class="palette__filter">
          <span class="palette__filter-label">Type</span>
          <span class="palette__filter-value">{view()?.fileFilter ?? "All Types"}</span>
        </div>
      </Show>
      <section class="palette__results" aria-live="polite">
        <Show when={failed()}>
          {(message) => <p class="palette__failed">{message()}</p>}
        </Show>
        <Show when={note()}>
          {(message) => <p class="palette__note">{message()}</p>}
        </Show>
        <Show when={!failed() && rows().length === 0 && mode() === "search"}>
          <p class="palette__empty">
            {query().trim() === "" ? "Type to search." : "Nothing matches."}
          </p>
        </Show>
        <ul class="palette__list">
          <For each={rows()}>
            {(row, index) => (
              <li
                class="palette__row"
                classList={{ "palette__row--selected": index() === selected() }}
                onMouseEnter={() => setSelected(index())}
                onClick={() => void activate(row)}
              >
                <span class="palette__name">{row.name}</span>
                <span class="palette__detail">{row.detail}</span>
                <Show when={isFile(row) && row.isDir}>
                  <span class="palette__tag">Folder</span>
                </Show>
              </li>
            )}
          </For>
        </ul>
      </section>
      <footer class="palette__footer">
        <Show
          when={mode() !== "search"}
          fallback={
            <>
              <span>↑↓ to choose</span>
              <span>↵ to open</span>
              <span>esc to hide</span>
              <button class="palette__link" type="button" onClick={() => void openFileSearch()}>
                Search Files
              </button>
            </>
          }
        >
          <Show when={mode() === "fields"} fallback={<span class="palette__confirm">↵ to run · esc to cancel</span>}>
            <span class="palette__confirm">tab for the next field · ↵ to run · esc to go back</span>
          </Show>
        </Show>
      </footer>
    </main>
  );
}
