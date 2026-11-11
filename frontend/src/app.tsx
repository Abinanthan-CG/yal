// The palette.
//
// One field at the top, the results under it. The window is shown and hidden
// rather than opened and closed, so the field is focused every time the
// palette appears.

import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { hidePalette, openResult, search, suggestions, type ResultRow } from "./api";

/** How long the typing rests before the launcher is asked. */
const SETTLE_MS = 60;

export function App() {
  const [query, setQuery] = createSignal("");
  const [rows, setRows] = createSignal<ResultRow[]>([]);
  const [selected, setSelected] = createSignal(0);
  const [failed, setFailed] = createSignal<string | null>(null);
  let input: HTMLInputElement | undefined;
  let pending: number | undefined;

  const ask = async (text: string): Promise<void> => {
    try {
      const found = text.trim() === "" ? await suggestions() : await search(text);
      setRows(found);
      setSelected(0);
      setFailed(null);
    } catch (error) {
      setRows([]);
      setFailed(String(error));
    }
  };

  // Every change of the field restarts the wait, so a fast typist asks once.
  createEffect(() => {
    const text = query();
    window.clearTimeout(pending);
    pending = window.setTimeout(() => {
      void ask(text);
    }, SETTLE_MS);
  });

  onMount(() => {
    input?.focus();
  });

  onCleanup(() => {
    window.clearTimeout(pending);
  });

  const move = (delta: number): void => {
    const count = rows().length;
    if (count === 0) {
      return;
    }
    setSelected((current) => (current + delta + count) % count);
  };

  const openSelected = async (): Promise<void> => {
    const row = rows()[selected()];
    if (!row) {
      return;
    }
    try {
      await openResult(row.id, query());
    } catch (error) {
      setFailed(String(error));
    }
  };

  const onKeyDown = (event: KeyboardEvent): void => {
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
      <section class="palette__results" aria-live="polite">
        <Show when={failed()}>
          {(message) => <p class="palette__failed">{message()}</p>}
        </Show>
        <Show when={!failed() && rows().length === 0}>
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
                onClick={() => void openResult(row.id, query())}
              >
                <span class="palette__name">{row.name}</span>
                <span class="palette__detail">{row.detail}</span>
              </li>
            )}
          </For>
        </ul>
      </section>
      <footer class="palette__footer">
        <span>↑↓ to choose</span>
        <span>↵ to open</span>
        <span>esc to hide</span>
      </footer>
    </main>
  );
}
