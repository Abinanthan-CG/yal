# 020 - The command catalogue, the custom commands, and the shell they run under

Date: 2026-11-13
Crates: `crates/tc-commands/` (`catalog.rs`, `model.rs`, `store.rs`, `shell.rs`,
`runner.rs`, `raycast.rs`, `arguments.rs`)
App: `apps/yal/src/launcher.rs`, `apps/yal/src/shell.rs`, `frontend/src/app.tsx`,
`frontend/src/api.ts`
Upstream: `Tinycast/Features/Launcher/Model/CommandCatalog.swift`,
`Tinycast/Features/Launcher/Model/CommandID.swift`,
`Tinycast/Features/CustomCommands/Model/CustomCommand.swift`,
`Tinycast/Features/CustomCommands/Model/RaycastScriptImport.swift`,
`Tinycast/Features/CustomCommands/Service/ShellCommandRunner.swift`,
`Tinycast/Features/CustomCommands/UI/CustomCommandArgumentsAccessory.swift`

## What this slice is

The command layer, in two halves: the **catalogue of built-in commands** and the
**user's own shell commands**. Upstream keeps the catalogue in the launcher and
the custom commands in their own feature; the port keeps the two rules in one
crate, `tc-commands`, because they share the command list, the panes and the
switches.

## The catalogue (`catalog.rs`)

Every built-in command sits in exactly one **Settings pane**, in the order the
documentation table shows it:

| The pane | The commands |
| --- | --- |
| AI | Quick AI, AI Chat |
| Quick Actions | Fix Grammar, Rewrite, Translate, Summarize |
| Clipboard | Clipboard History, Paste Sequentially |
| Emoji & Symbols | Search Emoji & Symbols |
| File Search | Search Files |
| Navigation | Switch Windows, Switch Windows Backwards, Search Menu Bar Items |
| Calendar | Join Next Meeting, My Schedule, Create Event, Copy Meeting Link, Open in Calendar |
| Notes | Show Notes, Create Note, Search Notes |
| Window Management | Create Window Layout, Create Layout from Current Windows, Switch Room, Create Room |
| Quicklinks | Create Quicklink, Search Quicklinks, Import Quicklinks, Export Quicklinks |
| Snippets | Search Snippets, Create Snippet |
| Commands | Create Custom Command, Search Custom Commands |

The eleven commands no feature pane owns - Calculator History, Define Word,
Open Camera, Export Backup, Import Backup, Import from Raycast, Check for
Updates, YAL Settings, About YAL, Support YAL, Quit YAL - are listed in
**Settings -> Commands** instead. The four that carry the app's own name carry
this port's name.

**The switches.**

- A feature that is off hides its commands *and* their shortcuts.
- `Enable Commands` turns off every command in the Commands pane, and their
  shortcuts with them.
- Clearing a row's checkbox hides that command from search; **its shortcut keeps
  working**.
- The two custom-command commands follow `Enable custom commands`, which is off
  by default, rather than `Enable Commands`.
- `Open in Browser` and `Run Shell Command` need text to act on: they are never
  listed, and a chord carries no query, so they have no shortcut either.
- `Quit YAL` has no shortcut, so a stray key press cannot quit the app.

**The shortcut's second press.** Most commands that open a screen work as
toggles - press the shortcut again and the screen closes. Switch Windows and
Switch Windows Backwards **cycle** the selection instead.

## The custom command (`model.rs`, `store.rs`)

The fields, their defaults and their rules are upstream's:

| The field | The default |
| --- | --- |
| Name, Command | required |
| Icon | the shared terminal symbol |
| Show in root search | on |
| Load shell environment | off |
| Needs confirmation | on |
| Show output | off |
| Show confirmation | off |
| Arguments | none, at most three |
| Run In | empty, which means the home folder |

Only the **name** is searchable, so a command cannot be run by half-remembering
its flags. A command that is switched off is never offered; taking it out of the
main search keeps its shortcut.

Upstream keeps the commands in `UserDefaults`; the port writes
`commands.json` beside the app's other data, with upstream's record shape, so a
backup written by either app can be read by the other.

## The shell (`shell.rs`, `runner.rs`)

Upstream runs `zsh -lc <command>`. Windows has more than one shell, so a command
names its own - **PowerShell** (the default), **cmd**, **Git Bash** or **WSL** -
and the rules are the same everywhere:

- the values are passed **positionally** and never spliced into the command
  text: `$1`...`$n` and `$args` in PowerShell, `%1`...`%n` in cmd, `$1`...`$n`
  in bash;
- the command runs with **no console window**;
- its **input is closed**, so anything that reads input gets an end of file;
- the **environment is inherited, plus `TINYCAST=1`**;
- it starts in its **Run In folder**, or the home folder; if that folder is
  gone, the command does not run and the app says which folder is missing;
- there is **no timeout**; the app never stops a running command by itself;
- the command and everything it started sit in a **job object**, so `Stop` ends
  the whole tree, and **quitting the app leaves the command running**.

The values reach the shell without ever being text:

```
$1 = $args[0]
$2 = $args[1]
Write-Output $1
```

A value like `; rm -rf ~` is a string the command reads, not a second command.

**Windows PowerShell's `-Command` cannot carry both the command text and the
values**, so the command is written to a temporary `.ps1` and run with `-File`;
the file is removed when the run ends. This is the one place where the command
text touches the disk, and it never stays there.

**Load shell environment** means loading the PowerShell profile (`-NoProfile` is
passed when it is off). Upstream looks for exit code 127; Windows PowerShell has
no exit code for a command it cannot find, so the port looks for the
command-not-found message instead and says the shell configuration was not
read.

## The Raycast import (`raycast.rs`)

A folder of Raycast script commands is read, and each script that has both a
shebang line and an `@raycast.title` becomes a custom command. The `@raycast.*`
block is mapped as upstream maps it - `title`, `mode`, `needsConfirmation`,
`argument1`...`argument3`, `currentDirectoryPath`.

The one Windows step is the interpreter: a Raycast shebang names a Unix path, so
the importer resolves the name against Windows - `python`, `node`, and `bash`
through Git Bash or WSL - and says plainly when it is missing.

## What is not done

- The **Settings panes** are not built. The rules the panes hold are in the
  model and tested; the screen that shows them is not there yet.
- There is no **output window**: the output is captured, the last 256 KB is
  kept, and the result and the duration are known, but nothing shows them, and
  the colours and the in-place progress bars need a pseudo-console.
- The **palette's own key handling** - the tab between the fields, the escape
  that leaves them - is in the window, which has no test yet.
- The **backup** does not carry the custom commands yet, and nothing warns
  before an import.

## Verified

- `cargo test --workspace` passes: 931 tests, 0 failed, 0 ignored. The command
  crate alone runs 88, of which 17 start a real process.
- `cargo clippy --workspace --all-targets -- -D warnings` passes, exit 0.
- `cargo fmt --all -- --check` passes, exit 0.
- `cargo run -q -p xtask -- parity` reports 2309 rows, 333 PORTED, 1976 NOT
  PORTED. The commands page alone moved 87 rows.
