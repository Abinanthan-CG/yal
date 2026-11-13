// The palette.
//
// One field at the top, the results under it. The window is shown and hidden
// rather than opened and closed, so the field is focused every time the
// palette appears.
//
// A row of the user's own command may ask for values first, and may have to
// be confirmed before it runs; both happen inside the palette.

import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import {
  hidePalette,
  openResult,
  runCustomCommand,
  search,
  suggestions,
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
  let input: HTMLInputElement | undefined;
  let timer: number | undefined;

  const ask = async (text: string): Promise<void> => {
    try {
      const found = text.trim() === "" ? await suggestions() : await search(text);
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
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        move(-1);
        break;
      case "Enter":
        event.preventDefault();
        void openSelected();
        break;
      case "Escape":
        event.preventDefault();
        void hidePalette();
        break;
      default:
        break;
    }
  };

  return (
    <main class="palette">
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
          onInput={(event) => setQuery(event.currentTarget.value)}
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
