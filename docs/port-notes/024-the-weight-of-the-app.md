# 024 - The weight of the app

Date: 2026-11-14 (numbers corrected 2026-11-15)
Subject: what YAL costs at runtime, measured against the app it is meant to replace

## Why this note exists

The whole reason this port exists is that Raycast is comfortable and Raycast is
heavy. A port that keeps the comfort and loses the lightness has missed the
point, so the resource budget is a feature requirement, not a nicety.

Until now that budget was a number in the imagination. This note records the
numbers as measured on the development machine, and what they say about the
architecture we chose.

## The measurement

`scripts/measure-resources.ps1` launches the release build, walks the whole
process tree (the app plus every WebView2 process it owns), and reports working
set, private bytes and idle CPU. The same numbers are printed for Raycast when
it is running, because that is the number we are trying to beat.

Two counting rules matter, and getting them wrong flatters the wrong app:

- **The whole tree counts.** Raycast does not only run `Raycast.exe`. It also
  owns a WebView2 (six processes) and two `Dimmer` helpers that dim the screen
  for its screenshot and AI features. Counting only the processes named
  `Raycast*` reports 331 MB and misses 580 MB.
- **Apps Raycast happened to launch are not Raycast.** On this machine Raycast
  started the editor this session runs in, so a naive tree walk pulls that
  whole tree in as well. The walk descends only through Raycast's own process
  names.

Measured on the development machine, Windows 11, release build of 2026-11-13:

| | Working set | Private bytes |
| --- | --- | --- |
| **Raycast, whole tree** | **914 MB** | 766 MB |
| Raycast.exe alone | 312 MB | 289 MB |
| Raycast's six WebView2 processes | 488 MB | 209 MB |
| Raycast's two Dimmer helpers | 102 MB | 175 MB |
| **YAL, whole tree, palette open** | **367 MB** | 174 MB |
| YAL's six WebView2 processes | 332 MB | 166 MB |
| `yal.exe` (the Rust core) | 25 MB | **6 MB** |

**YAL is already the lighter one** - about 2.5 times lighter than Raycast in
working set, and roughly four times lighter in private bytes. That is the
number the user was right to remember about Raycast, and the port is on the
right side of it.

## What the numbers say

**The Rust core is not the cost.** Everything YAL does - the calculator, the
clipboard watcher, the search index, the settings, the tray - lives in 6 MB of
private bytes. That is the part of the port that was written for this project.

**The WebView2 runtime is the cost.** Roughly 90% of YAL's memory is Chromium's
browser process, GPU process, renderer and utility processes, spawned before
the user has typed anything. Tauri starts the web view eagerly, so the price is
paid at launch and kept for the life of the process. Hiding the window saves
almost nothing: Chromium keeps its pages and its browser process resident.

**Raycast pays the same tax, and more.** It embeds a WebView2 too - six
processes, 488 MB - on top of a 312 MB native application and two screen-dimming
helpers. Raycast's interface is native; its web content is not.

**The idle CPU is Chromium, not our code.** The frontend has no animation loop,
no interval and no `requestAnimationFrame` (`frontend/src` has exactly one
match for `animation`, and it is a settings field name). A visible WebView2
window simply keeps producing frames. Measured idle CPU: YAL 0.3-6% of one
core depending on how the window was left, Raycast 0.6%.

## The budget we are aiming at

| State | Target |
| --- | --- |
| Resident, nothing open | **< 40 MB** private, < 0.5% of one core |
| Palette open, typing | < 120 MB private |
| Cold start to first paint | < 250 ms |

## The ways to get there

### 1. A lazy web view - and what it actually costs

Create the WebView2 window when the hotkey is pressed and destroy it when the
palette closes. Resident cost falls to the Rust core alone. The delay is the
obvious cost; it is not the only one.

- **The latency lands on the one interaction that matters.** A launcher is
  judged on how fast it opens. Raycast's native palette is on screen in tens
  of milliseconds; a fresh WebView2 needs its environment and browser process
  built first, which is a few hundred milliseconds warm and worse when the
  machine has been idle and Chromium's binaries are cold in the page cache.
- **The spike stays.** The transient 400 MB is still there every time the
  palette opens. Only the resident number improves, and the resident number is
  the one nobody watches while using the app.
- **The cost is paid per invocation.** A launcher is opened dozens of times a
  day. You would trade a permanent latency tax for a memory saving that is
  only visible while the app is idle.
- **IME and focus.** The palette is a text field that accepts typing at once.
  A brand-new WebView2 has to attach the Text Services Framework, take focus
  and prime the renderer; the first keystrokes can be dropped and the input
  candidate window can lag. For a keyboard launcher with Indic input on this
  machine, that is not a detail.
- **State across opens.** The query text, the selection and the scroll position
  live in the Solid app. Destroying the web view loses them, so each open has
  to restore them from Rust. That is the right place for them anyway, but it
  is work, and it is a source of bugs.
- **Lifecycle races in the hotkey path.** Create-and-destroy on every toggle is
  where tray apps acquire their famous bugs: the hotkey arrives while a
  destroy is in flight, or the window is gone while the app still believes it
  is open, and the launcher stops responding to the only key that matters.
- **WebView2 teardown is not reliably clean.** Chromium's browser process can
  outlive the last window, and repeated create/destroy cycles churn the GPU
  process. If the processes do not actually exit, the change buys a slow leak
  instead of a saving.

### 2. A native palette

Render the palette without a browser engine: a Rust window with a native text
renderer. A palette is a text field, a list and a keyboard; it does not need
Chromium. This lands near 25-40 MB in total, because the 6 MB Rust core is
then the whole application - roughly ten times lighter than the current build
and more than twenty times lighter than Raycast. It also opens in tens of
milliseconds, and it has no IME, focus or focus-stealing surprises.

The cost is a rewrite of the palette surface: the search field, the result list
and the settings panes, in a Rust UI toolkit. The engine crates are untouched -
the calculator, the clipboard, the snippets, the file index and the palette's
decisions all stay exactly as they are - and the Solid frontend becomes the
reference for behaviour rather than the runtime interface.

### 3. Trimming the runtime

WebView2's browser arguments (`--disable-gpu`, renderer process limits) can cut
tens of megabytes, at the cost of relying on flags the runtime does not
promise. A mitigation, not a fix.

## Where this leaves the port

Option 2 is the destination, because it is the only one that reaches the
budget outright, and the measurement says the weight is in the UI runtime
rather than in the engine. Option 1 is no longer the first move: it trades the
app's best quality - instant opening - for a resident number that only matters
while the app is not being used. Option 3 is a mitigation that can be applied
to whatever UI ends up shipping.

## What was measured, and what was not

Measured: process tree memory and idle CPU for both apps, the split between the
Rust core and the WebView2 processes, the effect of hiding the window, and the
difference between working set and private bytes.

Not measured yet: memory under a real workload (a populated file index, a
clipboard history with images, the search index at scale), disk footprint of the
stores, the cost of the features that are not built yet, and what a native
palette actually weighs - which is a number to take from a prototype, not to
project. Those will be measured with the same script once those things exist,
and the budget above will be checked against them rather than against an idle
window.
