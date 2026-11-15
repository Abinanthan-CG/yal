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
| the application, before the browser window was made on demand | 391.7 MB | 212.9 MB | 6 |
| **the application, now** | **58.8 MB** | **37.9 MB** | **1** |

The last row is the budget: under 40 MB of private bytes, one process, no
second runtime, 0.19 s of CPU to start and open the palette. The middle row
is what it was an hour earlier.

**The browser window was the whole of the difference.** Even hidden, Tauri
created the webview, and WebView2 started five more processes - the browser
host, the GPU process, the network service, the storage service and the
renderer - for a window nobody was looking at. That was 330 MB.

So the window is not in the configuration any more. It is made when the
settings screen is asked for, and it is made *before* the screen is
opened, so the browser shows the right screen the moment it appears. The
tray's Settings item opens it; `yal.exe --settings` opens it too, which is
how the path is checked.

The settings screens are still drawn by the browser. That is the next
step, not this one: the destination is a native settings pane, and until it
exists the browser is a cost the user pays only when they ask for it.

## Where the app stands against the app it replaces

The whole tree, measured the same way as
[port note 024](024-the-weight-of-the-app.md):

| | working set | processes |
|---|---|---|
| YAL, launching things | 58.8 MB | 1 |
| YAL, with the settings open | 391.7 MB | 6 |
| Raycast for Windows | 914 MB | several |

The common case is the one that matters, and it is now fifteen times
lighter than the app this port replaces.
