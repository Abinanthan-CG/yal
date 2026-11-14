# 024 - The weight of the app

Date: 2026-11-14
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

Measured on the development machine, Windows 11, release build of 2026-11-13:

| | Working set | Private bytes | Idle CPU |
| --- | --- | --- | --- |
| Raycast (running, window closed) | 331 MB | 277 MB | 1.4% of one core |
| YAL, palette window visible | 396 MB | 201 MB | 6.1% of one core |
| YAL, palette window hidden | 391 MB | 193 MB | 3.3% of one core |

Broken down, YAL's tree was:

| Process | Working set | Private bytes |
| --- | --- | --- |
| `yal.exe` (the Rust core) | 26 MB | **6.6 MB** |
| 7 × `msedgewebview2.exe` | ~365 MB | ~186 MB |

## What the numbers say

**The Rust core is not the cost.** Everything YAL does - the calculator, the
clipboard watcher, the search index, the settings, the tray - lives in 6.6 MB
of private bytes. That is the part of the port that was written for this
project, and it is already two orders of magnitude away from the problem.

**The WebView2 runtime is the cost.** Roughly 90% of the memory and all of the
trouble is Chromium's browser process, GPU process, renderer and utility
processes, spawned before the user has typed anything. Tauri starts the web
view eagerly, so the price is paid at launch and kept for the life of the
process. Hiding the window saves 0.5 MB and 2.8 points of CPU: Chromium keeps
its pages alive and its browser process resident.

**Today YAL is heavier than Raycast.** 396 MB against 331 MB, and six times the
idle CPU. The premise of the port is currently false, and no amount of care in
the Rust code changes that, because the weight is in the UI runtime.

**The idle CPU is Chromium, not our code.** The frontend has no animation loop,
no interval and no `requestAnimationFrame` (`frontend/src` has exactly one
match for `animation`, and it is a settings field name). A visible WebView2
window simply keeps producing frames.

## The budget we are aiming at

| State | Target |
| --- | --- |
| Resident, nothing open | **< 40 MB** private, < 0.5% of one core |
| Palette open, typing | < 120 MB private |
| Cold start to first paint | < 250 ms |

## The three ways to get there

1. **Native rendering instead of WebView2.** A palette is text, a list and a
   keyboard; it does not need a browser engine. Rendering it natively (a Rust
   window with a text renderer) lands near 20-30 MB total, because the 6.6 MB
   Rust core is then the whole app. This is the path that matches the promise,
   and it is a rewrite of the palette surface, not of the engine.

2. **A lazy web view.** Create the WebView2 window when the hotkey is pressed
   and destroy it when the palette closes. Resident cost drops to the Rust
   core alone; each invocation pays a WebView2 spin-up of a few hundred
   milliseconds and a transient 400 MB spike. Cheaper than a rewrite, but every
   keystroke-to-answer path pays the tax.

3. **Trim the runtime.** WebView2's browser arguments (`--disable-gpu`,
   renderer process limits) can cut tens of megabytes, at the cost of relying
   on flags the runtime does not promise. A mitigation, not a fix.

The honest reading of the measurement is that the WebView2 choice and the
lightness goal are in tension, and the tension is decided in the UI layer, not
in the Rust layer. Option 1 is the only one that reaches the budget outright.

## What was measured, and what was not

Measured: process tree memory and idle CPU for both apps, the split between the
Rust core and the WebView2 processes, and the effect of hiding the window.

Not measured yet: memory under a real workload (a populated file index, a
clipboard history with images, the search index at scale), disk footprint of the
stores, and the cost of the features that are not built yet. Those numbers will
be taken with the same script once those features exist, and the budget above
will be checked against them rather than against an idle window.
