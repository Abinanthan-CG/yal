# 028 - The palette that is the application

Date: 2026-11-15
Subject: the native palette stops being a probe and becomes the launcher

## What changed

Until now the native palette was a probe: a binary that opened a window with
ten fixed rows so its look and its weight could be measured. It was never
wired into the application. The application still ran on a browser window,
which is where 327 MB of the 391 MB the whole app tree costs goes.

Now the palette is the launcher. The hotkey opens it, the tray opens it,
it searches the program index, and the row the user chooses is run. The
picture is
[docs/port-notes/images/native-palette-in-app.png](images/native-palette-in-app.png),
taken from the running application, not from the probe.

## Three things the palette needed before it could be driven

**It had to be able to ask.** A window that only shows is a picture of a
window. `tc-ui` now has a small conversation:

```rust
match event {
    PaletteEvent::Shown => { /* put the suggestions back */ }
    PaletteEvent::QueryChanged(query) => { /* here is what was typed */ }
    PaletteEvent::Activated(index) => { /* this is the row that was chosen */ }
    PaletteEvent::Dismissed => { /* the user went away */ }
}
```

and the answer is a reply: nothing, a new set of rows, or close.

**It had to be able to be told.** The reply carries the model, because the
app is the one that knows what the content should be - the palette does not
search, it shows. The handler is moved out of the window's state for the
length of the call, so it cannot reach the palette's own state while it
runs; the model is put in place afterwards.

**It had to be able to change its size.** The window is the size of what it
shows, so a shorter list gets a shorter window. Before, the height was
computed once, when the window was made.

## Two bugs that only running it could find

Both were found by taking a screenshot of the running application and
noticing that the list was empty.

**The hint is not a query.** The search field shows "Search YAL" as
ordinary text, written into the control when it is made. Writing text into
an edit control raises `EN_CHANGE` - the same notification a keystroke
raises. So the palette told the app the query had become "Search YAL", and
the app searched for it, and found nothing.

The rule now: while the field is showing the hint, the app is not told
about the query at all. The user has not typed anything, so there is
nothing to report.

**An empty palette is not an empty list.** An empty query matches nothing -
that is right for a search, and wrong for a palette that has just been
opened. A palette that has just been opened shows what the user opens most;
typing narrows it down.

There is a third thing, which is a design rather than a bug: the palette
starts afresh every time it is shown. The field is emptied, the hint comes
back, and the app is asked for its suggestions again - which is what
`PaletteEvent::Shown` is for.

## One behaviour, two palettes

The browser palette and the native palette show the same rows, so a row has
to behave the same way in both. The behaviour moved into `apps/yal/src/open.rs`,
where the `Runner` holds the launcher, the window commands and the way to
quit, and `open(id, query, confirmed)` opens a row. The browser palette
calls it; so does the native one.

That is why the native palette is small: it has no rules of its own. It
searches with the launcher's search and runs rows with the launcher's
runner, both of which were already there and already tested.

## What it costs

Measured on the release build, on this machine:

| | working set | private | processes |
|---|---|---|---|
| the native palette alone | 57.7 MB | 36.8 MB | 1 |
| the application, native palette and all | 64.7 MB | 40.9 MB | 1 |
| the whole tree, with the browser window | 391.7 MB | 212.9 MB | 6 |

The first two rows are the budget: under 40 MB resident is met on private
bytes, one process, no second runtime. The third row is the problem, and it
is not the palette's problem.

**The browser window is still created at startup, and it costs 327 MB.**
Even though it is hidden, Tauri creates the webview, and WebView2 starts
six processes: the browser host, the GPU process, the network service, the
storage service, the crash handler and the renderer. The palette does not
need any of them. The settings screens still do, so the window has to
exist - but it does not have to exist at startup, and that is the next
step.

Until then, the honest statement is: the palette is light, and the
application that carries it is not, because it still carries a browser.
