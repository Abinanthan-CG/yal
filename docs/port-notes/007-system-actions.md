# 007 - The system actions, on Windows

Date: 2026-11-11
Crate: `crates/tc-platform-win` (`src/system_actions.rs`), `crates/tc-core`, `apps/yal`
Upstream: `Tinycast/Features/SystemActions/Model/SystemAction.swift`, `VolumeLevel.swift`

## What this slice is

Upstream lists thirty-two system actions — the ones that control the machine
itself rather than an app in it. The identifiers, the names, the groups and the
confirmations were already ported into `tc-core`; this slice makes them
**work**, and puts them in the palette.

The upstream source is macOS: `NSWorkspace` for the session, `AudioServices` for
the sound, the Finder for the Trash. Windows has a different answer for each of
them, and that answer is a table:

| Upstream's action | What YAL does on Windows |
| --- | --- |
| Lock Screen | `LockWorkStation()` |
| Sleep | `SetSuspendState(false, true, false)` |
| Sleep Displays | `SendMessage(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, 2)` |
| Restart / Shut Down / Log Out | `ExitWindowsEx` with `EWX_REBOOT` / `EWX_SHUTDOWN` / `EWX_LOGOFF` |
| Show Screen Saver | the shell opens `scrnsave.scr` |
| Play / Pause, Next, Previous | the media keys, pressed with `keybd_event` |
| Toggle Mute, Volume Up, Volume Down | the volume keys, pressed with `keybd_event` |
| Show Desktop | every top-level window is minimized |
| Toggle System Appearance | `AppsUseLightTheme` and `SystemUsesLightTheme` in the registry, then `WM_SETTINGCHANGE` |
| Hide All Apps Except Frontmost | every window but the frontmost is hidden with `ShowWindow(SW_HIDE)` |
| Unhide All Hidden Apps | every hidden window is shown again with `ShowWindow(SW_SHOW)` |
| Quit All Applications | every window is asked to close with `WM_CLOSE` |
| Open Trash | the shell opens `shell:RecycleBinFolder` |
| Empty Trash | `SHEmptyRecycleBinW` |

The table is a pure function, `mechanism(action)`, and it is written out in full
in a test. That test is the evidence behind the twenty rows that moved to
`PORTED`.

## The twelve Windows cannot do

Windows has no answer for these, and the port says so rather than doing
something else:

- **Set Volume, and the five fixed levels** — an absolute level needs the audio
  endpoint API (`IAudioEndpointVolume`), which is not wired up yet. The relative
  actions work, because the media keys are exactly the right key.
- **Stage Manager** — macOS only.
- **Dismiss Notifications** — Windows has no way for an application to clear
  the notification centre.
- **Eject All Disks** — needs the device installation API.
- **Toggle Hidden Files** — Explorer only reads the setting when it restarts, so
  a toggle would not be a toggle.
- **Toggle Bluetooth** — an application cannot switch the radio.
- **Toggle Microphone Mute** — an application cannot mute the microphone.

Each of them is a ledger row with a reason that says which of these it is.

## The Trash is the Recycle Bin

Upstream's Trash is the Finder's, and emptying it empties the Finder's bin.
YAL's is the **Recycle Bin**: "Open Trash" opens `shell:RecycleBinFolder`, and
"Empty Trash" calls `SHEmptyRecycleBinW`. A file in the Windows Recycle Bin is
still recoverable until that runs, which is the same promise the Finder makes.

## The confirmation

Upstream asks before five actions: Restart, Shut Down, Log Out, Empty Trash and
Quit All Applications. YAL asks too. The first Enter on such a row asks
("Press ↵ again to confirm") and the second one does it, and the Rust side
refuses to run an action that needs confirming until it is told the user has
confirmed — so the check is not only in the interface.

## The palette

The actions are rows in the palette under their own section, found by a word of
their name: "lock" finds "Lock Screen", "empty" finds "Empty Trash". A system
action is matched by a whole word rather than by scattered letters, because a
scattered match would make "vsc" find "Previous Track" and push the real Visual
Studio Code row off the screen.

## What was verified

- `cargo test --workspace`: 376 tests, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo fmt --all -- --check`: exit 0.
- `cargo run -p xtask -- parity --check`: the ledger and `PARITY.md` are up to date.

## A note on the workers

The snippets and clipboard slices were meant to be written by background worker
agents. Three attempts were aborted by the runtime — the session's own status
showed a provider bearer that was not synced, and one child session died with
"insufficient balance". The clipboard worker had already written a `lib.rs`
naming nine modules it never got to write, which left the crate uncompilable;
that was reverted, and the clipboard slice is not started.
