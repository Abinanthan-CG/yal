# Decisions

Dated entries. One paragraph each: the question, the options, what was chosen, and why.
Larger decisions get their own file in `docs/adr/`.

## 2026-11-10 - Ledger authoring is split into fragments

**Question.** The brief makes `parity/parity.toml` the single source of truth for scope and
status, but the ledger is far too large to write in one pass and it needs to be authored in
parallel.

**Options.** (a) Write `parity/parity.toml` directly. (b) Author per-group fragments under
`parity/parts/` and merge them into `parity/parity.toml`. (c) Generate the ledger from the
documentation pages with a script.

**Decision.** (b). `parity/parts/*.toml` are the authoring units; `cargo xtask parity` merges
them into the committed `parity/parity.toml`, validates it, and renders `PARITY.md` from it.
`cargo xtask parity --check` fails when either committed file is out of date, so CI catches a
stale ledger.

**Why.** (a) makes parallel authorship impossible without write conflicts on one file. (c)
cannot work: the atomic behaviors are only discoverable by reading the pages and the Swift
source, not by parsing them mechanically. The merged `parity/parity.toml` stays the single
source of truth that CI validates against, so the brief's requirement holds.

## 2026-11-10 - Count checks are a separate canonical file with two kinds

**Question.** The brief requires "every count check is satisfied", but upstream's count checks
are of two different shapes: some are literally a list of N items (32 system actions, 46
built-in commands), and some are a property a test must assert (a 250 ms tap window, a
32,000-character limit).

**Options.** (a) Model both as a row count. (b) Model both as free-form evidence on a row.
(c) Model them as two kinds in one canonical file.

**Decision.** (c). `parity/count_checks.toml` holds every check with `kind = "rows"` (the number
of tagged rows must equal `expected_rows`) or `kind = "asserted"` (a test must assert the value
later). Every check must be represented by at least one tagged row, so a check cannot be
declared and then quietly dropped.

**Why.** Treating an asserted limit as a row count would either force fake rows or force the
validator to accept an unchecked number. Two kinds let the validator be strict about the first
shape now and honest about the second shape later, without inventing a status the brief
forbids.

## 2026-11-10 - Upstream's `[[count_check]]` definitions live outside the ledger

**Question.** The brief's ledger schema has `count_check` entries inside `parity.toml`. Where
should they live so parallel authors cannot conflict?

**Options.** (a) Inside `parity/parity.toml`. (b) In `parity/count_checks.toml`, referenced by
id from rows.

**Decision.** (b). Rows carry `count_check = "<id>"`; the definitions live in one canonical
file. A fragment that defines a check which is not in the canonical file is a validation error.

**Why.** The count checks are a property of upstream's documentation, not of any one page
group, so they change as one unit. Splitting them across fragments would make the "46 built-in
commands" check editable from two places at once.

## 2026-11-10 - The Windows translation per page is data, not prose

**Question.** Each docs page needs a Windows translation, and the ledger's `windows` column must
be a per-behavior specialization of it rather than a generic restatement.

**Options.** (a) Keep the translation in prose in the prompt and re-derive it per row. (b) Store
the page-level translation in `parity/pages.toml` next to the page's tier and title.

**Decision.** (b). `parity/pages.toml` holds `page`, `title`, `tier`, `source` and `windows`.
The ledger rows specialize it.

**Why.** The page inventory, the tier and the translation change together and must be validated
as one unit: a row on an unknown page, or a row whose tier disagrees with its page, is an
error the validator can catch. Storing them together is what makes that check possible.

## 2026-11-10 - Deterministic TOML output for the ledger

**Question.** `cargo xtask parity --check` has to detect a stale committed ledger. What
guarantees the regenerated file matches byte for byte?

**Options.** (a) Serialize with `toml_edit`, whose formatting of table arrays is not
guaranteed stable. (b) Write the TOML by hand with an explicit string escaper.

**Decision.** (b). A small `tstr` function escapes TOML basic strings and the writer emits
sections in a fixed order: headings, rows, count checks, additions. `toml_edit`'s array API
does not accept tables in arrays in the form needed, and hand-writing removes the formatting
ambiguity entirely.

**Why.** The check has to be exact, and a ledger that reformats itself on every run produces
noise that hides real staleness.

## 2026-11-10 - Windows-dependent rows are provable here, not only in CI

**Question.** The brief says Windows-dependent rows become `PORTED` only once evidence has run
on real Windows. Is real Windows available?

**Options.** (a) Assume a non-Windows host and defer every Windows-dependent row to CI.
(b) Detect the environment and, when it is real Windows, prove those rows locally.

**Decision.** (b). The environment check in `PROGRESS.md` shows Windows 11 with the MSVC
toolchain, the Windows SDK, WebView2 and NSIS all present, so Windows-dependent rows can be
proven on this machine and CI is a second witness rather than the only one.

**Why.** The brief's `environment_adaptation` section says to find out where you are before
planning. Deferring everything to CI would have been correct but much slower, and would have
meant reporting far fewer rows as `PORTED` than the work actually justified.

## 2026-11-10 - The ledger's rows are generated, which replaces the hand-authored plan

**Question.** The first decision above split the ledger into fragments so that several people
could author it at once. Four parallel workers were dispatched to hand-write the rows. Three
produced nothing usable within the time limit, and the one that finished duplicated the
generated rows and carried a page-path typo.

**Options.** (a) Keep hand-authoring and fix the workers. (b) Generate the rows from the
upstream documentation, and hand-write only the status and the evidence.

**Decision.** (b). `cargo xtask ledger` derives the rows from the documentation pages: one row
per heading, table row, list item and paragraph, with `·`-separated lists split into one row
per item and shorthand percentages folded out. It writes `parity/parts/generated/*.toml`, which
is committed. The hand-authored fragments were deleted.

**Why.** The rows are a property of the documentation, not of any one author's judgement, so
generating them removes a whole class of error: a missed behaviour, a duplicated row, a typo in
a page path. The judgement that still needs a human — whether a behaviour is ported and which
test proves it — stays hand-written, and the validator still checks it.

**Effect on the earlier decision.** The fragments are still the authoring units, but they now
hold the *status* and the *evidence* rather than the behaviour text.

## 2026-11-10 - A running instance is asked to quit through a named event

**Question.** The installer replaces the program files, so it has to close a running instance
first. Upstream is a macOS application and has no equivalent problem.

**Options.** (a) Have the installer kill the process. (b) Have the installer ask politely
through a window message. (c) Use a named event that the running instance waits on.

**Decision.** (c). The running instance creates the named event `Local\YAL-quit` and waits on
it on a background thread; `yal.exe --quit-running` opens the same event and sets it. The
instance exits with code 0 when the event fires.

**Why.** (a) is rude and can lose the user's unsaved work. (b) needs the target window to
exist and to respond, which it may not. The event is per session and per user, so the signal
never reaches another user and never leaves the machine, and it works whether or not a window
is on screen.

## 2026-11-10 - The repository slug is written down once

**Question.** The updater, the packaging manifests and the documentation all need to name the
repository, and the slug is not decided yet.

**Options.** (a) Write the slug in each place that needs it. (b) Write it in one constant and
read it everywhere.

**Decision.** (b). `tc-core::REPOSITORY` holds the slug, and `tc-core::latest_release_url()`
builds the releases URL from it. `Cargo.toml` repeats it in `workspace.package.repository`
because Cargo has no way to read a constant.

**Why.** A slug that is written in five places is a slug that is wrong in two of them. One
constant means changing the repository name is a one-line change.

## 2026-11-10 - The upstream files are read with an explicit UTF-8 decode

**Question.** The upstream Swift sources and the documentation pages contain non-ASCII text:
arrows, multiplication signs, CJK and Cyrillic. Reading them shows mojibake.

**Options.** (a) Assume the files are broken and re-download them. (b) Read them with an
explicit UTF-8 decode.

**Decision.** (b). The files were downloaded with `Invoke-WebRequest -OutFile`, which writes the
raw bytes, and are read with an explicit UTF-8 decode.

**Why.** The first download used `Invoke-WebRequest` without `-OutFile`, which decoded the
responses as Windows-1252 and produced artifacts such as U+00E2 and U+0080. The files were
re-downloaded, and the reading tools that decode correctly are used from then on. The
mis-decoding is a property of the tool, not of the files, and mistaking it for corruption would
have led to re-downloading correct files forever.

## 2026-11-11 - The uninstaller keeps the user's data unless they ask otherwise

**Question.** The uninstaller removes the program files. Should it also remove the data folder?

**Options.** (a) Always remove it. (b) Always keep it. (c) Ask, and default to keeping it.

**Decision.** (c), with `/SD IDNO`. The uninstaller asks "Remove your YAL data as well?", and
the default answer is **No**, both for a silent uninstall and for a user who presses Enter without
reading.

**Why.** (a) destroys settings, quicklinks, snippets and notes without asking in the silent case,
which is the case most likely to be run by a script. (b) never cleans up, which leaves a folder
the user did not ask for. Asking, and defaulting to the answer that loses nothing, is the only
one of the three that cannot destroy work.

## 2026-11-11 - The application creates its own data folder at startup

**Question.** The uninstaller keeps the data folder, but the application never created one — the
folder only appeared when something wrote to it, and in the first release nothing does.

**Options.** (a) Create the folders lazily, when the first thing is written. (b) Create them at
startup.

**Decision.** (b). The shell creates the data folder and the cache folder in its setup, before
the palette is shown.

**Why.** (a) means the folders appear at unpredictable moments, which makes them harder to back
up and harder to reason about, and it leaves the uninstaller's promise — "your data survives" —
untestable when there is no data. Creating them at startup makes the layout of the machine
predictable, which is what a launcher is for.

## 2026-11-11 - The smoke test writes a marker rather than checking an empty folder

**Question.** The smoke test asserted that the data folder survived the uninstall. It failed,
because the folder had never existed.

**Options.** (a) Assert the folder exists. (b) Write a marker file before the uninstall and
assert that the marker survives.

**Decision.** (b).

**Why.** (a) passes for the wrong reason: an empty folder that was never there satisfies "the
folder is still there" only by accident, and it cannot tell "the uninstaller kept the data" from
"there was no data". A marker file is a real thing to preserve, so the check says what it means.

## 2026-11-11 - The ledger's scope is generated, its status is written by hand

**Question.** The ledger's rows come from upstream's documentation, which changes when upstream
changes. The port's verdict on each row is the port's own state. Where does each live?

**Options.** (a) Write the rows and the status together, by hand. (b) Generate the rows, and keep
the status in a second file that the generator reads and the validator checks.

**Decision.** (b). `cargo xtask ledger` generates the scope into `parity/parts/generated/`, and
`parity/status.toml` holds one `[[status]]` entry per row: its status, its evidence, and, when it
is not ported, the reason. The generator applies the status file on top of the generated scope, so
regenerating the scope never loses the port's own verdict.

**Why.** (a) mixes two things that change for different reasons: upstream's documentation changes
when upstream ships, and the port's verdict changes when the port ships. Writing them together
means every upstream documentation change asks the porter to re-read the whole ledger. With the
two apart, a status entry is only touched when the port itself changes, and a status entry naming
a row that no longer exists is reported as a problem rather than silently dropped.

The rows that have not been reached yet take their reason from `parity/reasons.toml`, one line per
page. A page's reason names the piece that is missing — "Favorites have no store and no rows in
the palette yet" — rather than shrugging with "not yet implemented", which is the same sentence on
all 2,297 open rows and therefore says nothing about any of them.

## 2026-11-11 - A row is ported when a test shows the behavior, not when the mechanism exists

**Question.** Upstream's documentation rows describe behaviors, but many of them are one sentence
that mixes a rule with the place the data comes from. How is a row judged?

**Options.** (a) A row is ported when the code that would implement it exists. (b) A row is ported
when a test demonstrates the behavior the row describes.

**Decision.** (b). Evidence is a test that ran, named as `crate::module::test_name`; the validator
refuses a row whose evidence names a crate that does not exist or a function that is not there.

**Why.** (a) lets a claim drift: a matcher that can score an alias is not the same as a launcher
that finds an entry by that alias, and the difference is exactly what a reader of `PARITY.md` is
asking about. (b) forces the distinction to be made in the open, one row at a time, and it makes
the ledger impossible to fill in with hope. It also makes the `NOT PORTED` reasons useful: they
name the piece that is missing rather than saying "not yet implemented".

## 2026-11-11 - Windows has no ICU, so each script gets its own table

**Question.** Upstream reads a name in another script with ICU's `toLatin` transform. Windows
ships no ICU.

**Options.** (a) Bundle ICU. (b) Use one transliteration library for every script. (c) Write the
tables the scripts need, and leave the rest alone.

**Decision.** (c), with one exception: Han is read with the `pinyin` crate, which is a table
underneath. Japanese kana, Hangul, Cyrillic and Greek each have an explicit table. Every other
script is not romanized at all.

**Why.** (a) would add a very large dependency to a launcher that has to start in milliseconds.
(b) is what ICU already is, and no single small library covers kana, Hangul, Cyrillic and Greek
at the quality upstream needs. Leaving the other scripts alone costs little: a name in Thai or
Hebrew is still an exact hit in its own script, which is the behavior that matters, and the
documented rule is that the Latin spelling is an extra, not a requirement.

## 2026-11-11 - A saved float comes back as the same float

**Question.** A learned ranking is written to `launcher-ranking.json` and read back. The score is
a double. After a round trip through JSON the score was one unit in the last place away from the
one that was written, and upstream compares the two exactly.

**Options.** (a) Compare the two with a tolerance. (b) Store the score as a string. (c) Make the
JSON round trip exact.

**Decision.** (c). `serde_json`'s `float_roundtrip` feature is enabled for the whole workspace, so
a persisted double parses back to the bits it was written from.

**Why.** (a) hides the difference rather than removing it, and a tolerance that is wide enough to
be harmless is also wide enough to swallow a real drift. (b) changes the file's shape away from
upstream's, for no gain. (c) is a one-line change with no runtime cost: the default `serde_json`
number parser is an approximation, and the feature turns on the correctly-rounded one.

## 2026-11-11 - Names are compared the way a file list compares them

**Question.** Upstream breaks a tie between two entries with
`localizedStandardCompare`, which is numeric and case-blind, so `Item 2` comes before `Item 10`.
Rust's `str` ordering is neither.

**Options.** (a) Use `str::cmp`. (b) Use a natural-ordering crate. (c) Write the comparison.

**Decision.** (c). `tc-search::order` splits both names into runs of digits and runs of other
characters, compares the digit runs as numbers, and compares the other runs without regard to
case.

**Why.** (a) puts `Item 10` first, which reads as a bug to anyone who has used a file list. (b)
adds a dependency for forty lines of behavior, and the crate would still need case-blindness
added on top. (c) is the behavior upstream documents, and it is the only place in the port where
two entries are otherwise indistinguishable.

## 2026-11-11 - The program index asks three places, because Windows has three

**Question.** Upstream finds applications by walking LaunchServices for `.app` bundles. Windows
has no bundle, and no single place that lists everything the user can open.

**Options.** (a) Walk the Start Menu only. (b) Read the registry's uninstall key only. (c) Read
all three places Windows keeps them, and merge.

**Decision.** (c). The **Start Menu** holds the shortcut and the name Windows shows the user. The
registry's **uninstall** key names every installed program, with its publisher and its version.
The registry's **App Paths** says how Windows starts a program that has no shortcut.

**Why.** (a) misses everything installed without a shortcut, which on a fresh machine is most of
it. (b) misses the programs a user launches daily but never installed deliberately, and its
display names are the installer's, not the user's. (c) is the union: the Start Menu's name wins
where there is one, the registry fills the gaps, and App Paths catches the rest.

## 2026-11-11 - A shortcut is opened as a shortcut

**Question.** A Start Menu entry is a `.lnk` file that points at an executable. To open the
program, the launcher could resolve the shortcut, or open the shortcut.

**Options.** (a) Resolve the shortcut with `IShellLinkW` and start the executable. (b) Hand the
shortcut itself to the shell.

**Decision.** (b). The shell opens a `.lnk` perfectly well, so nothing has to resolve it.

**Why.** (a) needs a COM apartment and the `IShellLinkW` interface, for no gain: the shell does
the same work, better, because it is the thing that knows what a shortcut means. It also keeps
the port's COM surface small, which matters in a process that has to start in milliseconds.

## 2026-11-11 - Programs with the same name are one entry

**Question.** Upstream keeps every installed copy of an application: each path appears in the
search, and the version and the containing folder help tell them apart. Windows machines have
the same problem: an x64 and an x86 copy, a machine-wide and a per-user install.

**Options.** (a) Keep every copy, as upstream does. (b) Merge the copies into one entry.

**Decision.** (b). Entries whose names fold the same are one entry, and the Start Menu's copy
wins. The registry's version number is kept as an alternate name, so the copies are still told
apart in the text.

**Why.** Upstream can afford to show three copies because it also implements the rules that
make them bearable: which copy the favorite points at, which copy the shortcut opens, and which
copy the uninstaller refuses to touch. Without those rules, three copies are three confusing
rows. One entry with the version in its name is honest and useful, and the version is still
there.

## 2026-11-11 - The calculator asks a catalog about the units and the currencies

**Question.** Upstream's calculator engine carries the unit catalog and the currency table
inside itself. Ours cannot, because neither is ported yet, and the unit catalog alone is
several thousand lines.

**Options.** (a) Port the units and the currencies first, and write the engine once. (b) Give
the engine a `Catalog` interface and let it answer `Unsupported` for a query it cannot finish.

**Decision.** (b). The engine asks the catalog whether a word is a unit or a currency, and a
query that needs a conversion it cannot make answers `CalcOutcome::Unsupported { area, detail }`
— a real answer, never a made-up number.

**Why.** (a) would put the whole calculator behind its largest piece, and the arithmetic would
go untested until the units arrived. With (b) the arithmetic core is ported and pinned by 52
tests now, the seam is named, and the unit catalog can be added later as a second implementation
of `Catalog` without touching the engine. A query for `10 kg` says "unsupported" instead of
answering wrongly, which is the same rule the whole project follows: say what is missing rather
than guess.

## 2026-11-11 - A crate that compiles is not a crate that is tested

**Question.** The calculator's first draft had 47 tests in `crates/tc-calc/src/tests.rs` and
`cargo test` reported "0 passed". The crate compiled, the doc test passed, and the tests did not
run: the module was never declared in `lib.rs`.

**Decision.** Every crate's tests are declared and the reported count is checked. The port note
for the slice says this in full.

**Why.** A green `cargo build` says nothing about the tests, and "the test file exists" is not
"the test ran". The whole ledger rests on evidence that actually executed, so a test that never
runs is worse than no test at all: it looks like proof. This is the same reason the parity
validator refuses a `PORTED` row whose evidence names a function that is not in the crate.

## 2026-11-11 - The other AI's port was reviewed, and nothing was taken from it

**Question.** A second YAL port was supplied as a source zip, claiming a full port with a few
features missing. Could any of it be salvaged?

**Options.** (a) Take the parts that look real and re-verify them. (b) Review it, write down
what it actually contains, and take nothing.

**Decision.** (b). The review is in `docs/audits/2026-11-11-other-port-review.md`.

**Why.** It does not compile (three of its binaries fail on a format string), its Windows layer
is empty (the credential store reports success while discarding the secret), and its "1,209
passing tests" are a JavaScript file that never runs the Rust — the calculator part reads the
answers out of a JSON file and asserts the file against itself. Its ledger has 1,208 rows
against this port's 2,309, so the two verdicts cannot even be compared row for row. Every
candidate piece would have to be re-verified against upstream's Swift before it could be called
ported, and at that point it is a port of upstream rather than a salvage.

## 2026-11-11 - A Windows action that cannot be done says so, and does nothing else

**Question.** Twelve of upstream's thirty-two system actions have no Windows answer: an
absolute volume level, Stage Manager, the notification centre, ejecting a disk, the
hidden-files toggle, the Bluetooth radio, the microphone. What should YAL do with them?

**Options.** (a) Approximate each one with the nearest Windows thing. (b) Leave them out of the
list. (c) Keep them in the list, answer "Windows cannot do that", and do nothing else.

**Decision.** (c). `Mechanism::Unavailable(reason)` carries the reason, the palette says it, and
nothing happens.

**Why.** (a) would be a lie: a "Toggle Bluetooth" that opens the Bluetooth settings page is not
a toggle, and a "Toggle Hidden Files" that needs Explorer to restart is not a toggle either.
(b) would make the list shorter than upstream's without saying why. (c) keeps the thirty-two
rows honest — the ledger says which twenty run and names the reason for each of the twelve that
do not.

## 2026-11-11 - The system actions are matched by a whole word

**Question.** The launcher matches a program by scattered letters, so "vsc" finds "Visual Studio
Code". Should the system actions be matched the same way?

**Decision.** No. A system action is found when the query appears in its name as a whole:
"lock" finds "Lock Screen", "vsc" finds no action at all.

**Why.** The scattered rule made "vsc" find "Previous Track" as well, which pushed the real
Visual Studio Code row out of a result list the user was reading to launch a code editor. The
programs keep the fuzzy rule, because that is upstream's rule and upstream's tests pin it; the
system actions are a list of thirty-two fixed names, and a word of the name is enough.

## 2026-11-11 - The port is done in this thread when the workers keep dying

**Question.** Three background worker attempts at the snippets and clipboard slices were aborted
by the runtime: one died with a connection error, one with "insufficient balance", and the
session's own status showed a provider bearer that was not synced. The workers read the big
upstream files so their contents never land in this conversation, which is what was wanted.

**Options.** (a) Keep retrying the workers. (b) Do the slices in this thread, reading upstream
with targeted greps.

**Decision.** (b), for now. The calculator slice was finished by a worker that did survive, so
the approach is not wrong in itself — but three aborts in a row is a signal, and a half-written
crate left in the tree is worse than no crate.

**Why.** The cost of the bulk read is real, but the cost of a worker dying half-way through a
crate is higher: the clipboard worker left a `lib.rs` naming nine modules it never wrote, which
did not compile until it was reverted. The upstream files can be read in pieces with targeted
greps, which keeps the context small without depending on a worker that may not finish.

## 2026-11-11 - The palette's build is not committed

**Question.** Tauri loads the palette's page from `frontend/dist/`. That directory can be
committed, or built.

**Options.** (a) Commit `frontend/dist/`. (b) Build it in every build, and in CI.

**Decision.** (b). `frontend/dist/` is a build output and is not committed. The sources are in
`frontend/src/`, and `scripts/build.ps1` builds the palette before it builds the Rust.

**Why.** (a) means two copies of the same interface, one of which is stale the moment anyone
edits the sources, and the stale one is the one that ships unless someone remembers to rebuild.
CI checks the types and builds the page on Linux, and builds it again on Windows, where the
installer is made from what was built. A committed build output is a claim that cannot be
checked; a built one always is.
## 2026-11-11 - The scope of YAL, decided by the person who will use it

**The question.** The ledger holds 2 309 rows, of which 120 are ported. Every remaining
row was listed - 348 features across 42 pages - and the scope was decided page by page.

**Decision.** 623 rows are out of scope; 1 566 are to be built.

| Cut | Rows | Why |
| --- | --- | --- |
| The AI features (the AI page, the MCP page, the quick actions, and the AI rows inside the commands and the shortcut reference) | 245 | YAL talks to no model provider and holds no API key. |
| The Raycast extensions, and Apple Shortcuts | 117 | macOS concepts with no Windows answer. |
| The macOS permissions | 46 | The accessibility and screen-recording permissions have no Windows answer. |
| The saved window layouts and the rooms that hold them | 70 | The window management itself stays; the saved layouts do not. |
| Dictation | 45 | Windows' own speech input does the job. |
| The in-app uninstaller | 37 | The installer still removes the program and keeps the data. |
| The update check | 25 | YAL is built and used by one person, who rebuilds it from the source. |
| The camera | 14 | Not for now. |
| The dictionary | 13 | The Define Word command opens a web lookup instead. |
| Reading an app's menu bar | 2 | macOS has a menu bar; Windows does not. |

**How a cut is recorded.** A cut is a decision, not a failure: the row stays in the ledger and
stays `NOT PORTED`, and its reason says that it was decided rather than forgotten. The cuts
live in `parity/status.toml` as `[[cut]]` entries, which name a page, a heading or a row.
Deleting a cut puts the feature back in scope, and the scope review is regenerated from the
ledger with `scripts/make-scope-review.ps1`.

**Why the ledger needed a new mechanism.** The reason a row is open used to be per page, which
could not say "the AI commands inside the commands page are out of scope while the rest of
the page is not". The cut entries give every row its own honest reason.

**What the calendar does on Windows.** Upstream's calendar is macOS EventKit. The Windows
answer is the calendar through WinRT, which reads the calendars the user already has in
Windows - and a command that opens the web calendar in the browser. A web view of Google
Calendar on its own would be a picture of a calendar: it could not answer "join the next
meeting" or "what is my schedule", and it would need the user to sign in to Google inside
our window.

**What "the palette" is.** Upstream's palette is the floating search panel the whole app
lives in - the launcher is its root screen, and every other screen stacks on it. The rows
under it are the back stack, the actions menu, the tab cycle and the key handling. It is the
app's own shell, and YAL has a first version of it and needs the rest.

**What the shortcut reference is.** Not macros. It is which key does what, per screen: return
pastes in the clipboard, command-return copies, the number keys open the favorites. The
shortcuts you record yourself are the separate hotkeys page.

## 2026-11-11 - We take machinery from the parallel YAL project, and only machinery

**Question.** The user pointed out a second YAL on this machine - `C:\Users\dabin\development\yal`,
the community project at `github.com/Abinanthan-CG/yal` - which builds the same product a
different way and was left in a corner. Should we use it, and how much of it?

**Options.** (a) Ignore it: it is another product's code and our brief says upstream's Swift
is the authority. (b) Port it wholesale, replacing the parts of YAL we have already built and
tested. (c) Take the proven algorithms from the parts that match our scope, and nothing else.

**Decision.** (c). Four pieces are taken: the incremental file-index walker (nanosecond
directory mtime freshness, reparse-point rejection, exact per-directory deletion), the window
placement geometry, the currency registry with its rate vintage, and the natural-language
phrase pre-parser in front of one evaluator. Each adapted module states its origin in its
module doc-comment and carries the donor's GPL-3.0 notice (`crates/tc-files/ATTRIBUTION.md`).
The egui UI is not ported - YAL is Tauri 2 and Solid by decision - and the donor's extra
features (QR, TOTP, password generator, regex, colour picker, screenshots, fonts, timers,
reminders, scripts, bookmarks) are not built, because upstream Tinycast has no such features
and they would be scope creep against the 2026-11-11 scope decision.

**Why.** (a) throws away machinery that has already paid for its bugs - the donor's index
walk was measured at 159 524 files cold in 52 s and warm under a second, and it already
solved the millisecond-floor and junction bugs we would otherwise meet on our first real
disk. (b) would trade 120 tested rows and a Tauri shell for an egui rewrite, and would put
a GPL-3.0 UI in front of a Solid frontend. (c) takes what is proven and keeps upstream's
behaviour as the specification: where the donor's product model differs from upstream's -
whole fixed drives against user-chosen scopes, for instance - upstream wins. A copied
algorithm still needs its own tests against upstream's behaviour before its rows may be
called `PORTED`.

**What this does not change.** No scope, no order, no milestones: the daily driver, then the
memory features, then the long tail. The saving is real but modest - about 18-28 hours of
roughly 330, or 5-8%, and the full reasoning with the verified evidence is in
`docs/audits/2026-11-11-parallel-yal-review.md`.
## 2026-11-12 - The port is published from `Abinanthan-CG/yal`, over the earlier build

**Question.** The other YAL project - the one at `C:\Users\dabin\development\yal` - was written
by the same person who is writing this port, and its repository is `github.com/Abinanthan-CG/yal`.
Should this port be published under a different name and leave that repository alone, or should it
take that repository's place?

**Options.** (a) Publish under a new repository, leaving the earlier build where it is. (b) Publish
under `Abinanthan-CG/yal`, replacing its contents, and keep the notices so the reused code says
where it came from.

**Decision.** (b). The repository slug is written down once, in `tc-core::REPOSITORY`, and is
`Abinanthan-CG/yal`; the `origin` remote is set to it and the branch is `main`, which is that
repository's default branch. The flush happens **at the end of the build**, not now, and it is one
confirmed action with the procedure written down in `docs/PUBLISHING.md`.

**Why.** Two builds of one product under two names is worse for the person who will use it than
one repository that holds the finished thing: the earlier build's issues, tags and README point at
a launcher that is not the one being maintained. The earlier build had no release and no tags, so
nothing installable disappears when it is replaced, and its history stays on this machine in
`C:\Users\dabin\development\yal`. The notices stay either way - they cost nothing, and they say
which code came from where, which is worth more than the licence question, since the donor is the
same person.

**What this does not change.** The scope, the order and the milestones. The flush is the last step
of Phase 4, after the tests, the ledger, the builds and the checksums are all in place.

## 2026-11-13 - The currency table is transcribed, not typed

**Question.** Upstream's currency table is generated: `Scripts/gen-currencies.js` writes
`CurrencyData.generated.swift` from the CLDR data, and it holds 159 fiat currencies, 26 signs and
130 nouns. How should the port hold it?

**Options.** (a) Type the table into Rust by hand. (b) Write the table into Rust by hand once and
never touch it again. (c) Transcribe upstream's generated file with a script, so the two can be
regenerated together.

**Decision.** (c). `scripts/gen-currency-table.ps1` reads upstream's generated Swift and writes
`crates/tc-calc/src/currency/table.rs`; the hand-written tables - the contested nouns, the ISO
names, the codes people spell from the sign, and the coins - are written out in
`crates/tc-calc/src/currency.rs`, because upstream writes those by hand too.

**Why.** 159 currencies typed by hand is 159 chances to spell a name wrongly, and no way to notice.
The generated file is the authority, and a script keeps the port's copy honest: when upstream's
CLDR data moves, one command moves ours with it. The script is part of the repository, so the
table's origin is written down where the table is.

**What this does not change.** The rates. Upstream's `CurrencyRateStore` and its feed are a
separate piece of work, and until they are here the port answers an amount of money and refuses to
convert between two currencies rather than answer a made-up number.

## 2026-11-13 - A calculation that mixes two currencies earns no card

**Question.** Upstream fetches the exchange rates and converts. The port has no rate feed yet. What
should `100 usd + 5 eur` answer?

**Options.** (a) Answer with a made-up conversion. (b) Answer the error card upstream shows when
the rates are missing. (c) Earn no card.

**Decision.** (c), for now. A single currency is carried through the calculation and shown as
money - `100 usd + 50` is `150.00 USD` - and two *different* currencies in one calculation earn
no card at all, because any number would be invented.

**Why.** The port's rule is that it never answers a made-up number. Upstream's error card is
"Exchange rates unavailable - check your connection", which is a true statement about a machine
that is online; here the honest statement is that the feature is not built, and no card says that
better than silence. The rows that need the rates stay `NOT PORTED` with their reasons.

**What this does not change.** The decision to build the rate feed, which is still open, and the
shape of the answer when it arrives: the currency conversion rows are theirs to move.

## 2026-11-13 - The calculator tries every layer before it gives up

**Question.** This port's engine stopped at the first layer that reported an area it could not
answer - a unit or a currency - and said so. Upstream's engine tries every layer and only gives up
when none of them answers. `5 as multiplier of 1` has a word the arithmetic reads as a unit (`as`
is the attosecond), so the port answered "the units are not supported" where upstream answers `5x`.
Which is right?

**Options.** (a) Keep stopping at the first area: it is the honest answer for a query that needs a
unit. (b) Try every layer, and report the area only when nothing else answers, as upstream does.

**Decision.** (b). The engine keeps the area it could not answer and reports it only when no layer
produces a card.

**Why.** The area is not the answer to every query that mentions it: a word that *looks* like a unit
is not always one. Upstream's parser attaches the unit and carries on, so a query that turns out not
to be arithmetic falls through to the phrase layers - and the phrase layers are what answer `5 as
multiplier of 1`. The port's answer for a query that really needs a unit is unchanged: `10 kg` still
says the units are not supported, because no layer answers it.

**What this does not change.** The arithmetic that carries a unit is still not built. The change
makes the port answer the phrases it can answer, not make it answer the unit arithmetic.

## 2026-11-13 - The command's text is written to a temporary file for PowerShell

**Question.** Windows PowerShell's `-Command` cannot carry both the command text and the values
that go with it: everything after the command string is read as part of the command. The port has
to pass the values positionally and never splice them into the text. Where does the command text
live?

**Options.** (a) Put the values in environment variables and read them in the command text. (b)
Write the command to a temporary `.ps1` and run it with `-File`, so the values arrive in
`$args` and the first lines bind them to `$1`, `$2` .... (c) Put the values in the command
line and quote them there.

**Decision.** (b). The command text goes to a temporary `.ps1` file, which is removed when the
run ends. The values arrive after the file name, so they are never part of the text: a value like
`; rm -rf ~` is a string the command reads.

**Why.** (a) would make `$args` empty, and the documentation promises `$args` as well as
`$1`. (c) is the thing the port promises not to do. Upstream puts the command in zsh's argument
list, which is visible in the process list; the temporary file is no more visible than that, and it
is deleted rather than left behind.

**What this does not change.** The promise that the command text is never written to a log. The
file is not a log, it is the program being run, and it does not survive the run.

## 2026-11-13 - Needs confirmation is on by default, as the documentation says

**Question.** Upstream's documentation says a new command asks before it runs - "Needs
confirmation - On by default" - but upstream's code creates a new command with the flag **off**.
Which is the port's default?

**Options.** (a) Follow the code: off. (b) Follow the documentation: on.

**Decision.** (b). A new command asks before it runs.

**Why.** The documentation is what the parity ledger measures, and it is the behaviour a user
reading the editor table expects. The port's rule is that a cut or a contradiction is recorded
rather than quietly resolved, so this is recorded here: upstream's own documentation and upstream's
own code disagree about this one default, and the port follows the documentation.

**What this does not change.** The row about the editor's field, which stays `NOT PORTED` until
the editor screen exists; the default itself is in the model and is tested.

## 2026-11-13 - The launcher offers only the built-in commands the build can run

**Question.** Upstream lists all forty-six built-in commands in the launcher's results. Most of
this port's features are not built, so a row for `Search Notes` would open a screen that does
not exist. What does the launcher offer?

**Options.** (a) List every built-in command, and say "not in this build yet" when one is opened.
(b) List only the commands the build can run, and keep the whole catalogue in the settings.

**Decision.** (b). The catalogue holds all forty-six commands, in their panes, with their rules;
the launcher offers the ones the build can run - at the moment Quit YAL and Support YAL - and the
rest arrive with their features.

**Why.** A result that cannot be opened is a dead end, and a launcher full of dead ends is worse
than a launcher that is quiet. Upstream's own rule is the same: a command whose feature is off is
not listed.

**What this does not change.** The catalogue rows, which are about the pane a command belongs to
and not about the launcher; and the rule that a feature that is off hides its commands and their
shortcuts.
## 2026-11-13 - The palette's decisions live in Rust, not in the window

**Question.** A key's meaning - what <kbd>esc</kbd> does, what <kbd>tab</kbd>
does, which screen is on top - was decided in the window, in TypeScript. The
frontend has no test framework, so none of those decisions could be tested,
and the parity ledger could only ever say `NOT PORTED` about the palette.
Where do the decisions live?

**Options.** (a) Keep them in the window and add a test framework for the
frontend. (b) Move them into Rust and let the window ask what a key means.

**Decision.** (b). `crates/tc-palette` holds the screens, the keys, the
settings and the state machine; the window sends the key and is told what to
do about it.

**Why.** A decision that cannot be tested is not a decision, and the ledger's
rule is that a row is `PORTED` only when the evidence is a test that actually
ran. The decisions are also worth testing on their own: the escape chain, the
tab cycle and the emacs chords are three small state machines, and they are
much easier to pin down in Rust than through a browser. The window is now a
renderer of what the Rust side says, which is also the shape the rest of the
port already has.

**What this does not change.** The frontend still owns the text in the search
field and the row that is selected; the Rust side is told the text with every
key and holds the screens, the stack, the filter and the settings.

## 2026-11-13 - A screen offers only the actions the build can run

**Question.** Upstream's file-search screen has nine actions. Three of them -
copying the file itself, pasting it into another application, and adding it
to the favorites - are not built in this port. Does the screen still list
them?

**Options.** (a) List all nine, and say so when one is opened. (b) List only
the six that work.

**Decision.** (b). The file screen's actions menu lists Open File / Open
Folder, Show in File Explorer, Filter by Type, Copy Name, Copy Path and Move
to the Recycle Bin - and nothing else.

**Why.** The same rule the launcher already follows: a result that cannot be
opened is a dead end, and a dead end is worse than a missing row. The rule
that a command whose feature is off is not listed is upstream's own rule. A
test holds it: every action the file screen lists is one this build can run.

**What this does not change.** The three actions themselves, which stay in
the ledger as `NOT PORTED` with the reason that they are not built; and the
recording of the shortcuts, which are still in `docs/SHORTCUTS.md` with a note
that the screen does not offer them yet.

## 2026-11-13 - The General pane shows six settings, and the rest of the settings are not there yet

**Question.** <kbd>Ctrl</kbd>+<kbd>,</kbd> opens the settings screen. What is
in it?

**Options.** (a) The whole of upstream's settings. (b) The General pane's
appearance settings only, and the other panes later.

**Decision.** (b). The General pane shows the appearance, the animations, the
interface size, the window mode, the escape behaviour and the pop-to-root
delay. Each one moves to its next value on return, and each one is written
down and read back when the app starts.

**Why.** A settings screen with nothing in it is a dead end, and one that
shows a setting it cannot change is half a feature. The six settings above
are the ones the palette itself follows, so each one has a visible effect:
the theme, the size and the window mode change the palette, and the escape
behaviour and the pop-to-root delay change what a key does. The panes that
need an editor - the custom commands, the snippets, the quicklinks - are the
next slice, and they are not faked with an empty pane.

**What this does not change.** The other settings - the compact shortcuts,
the placement, the input source, the file-search scopes - which stay in the
model, are tested there, and stay `NOT PORTED` in the ledger because they are
not in a pane the user can reach.
## 2026-11-13 - A value carries its unit, and the catalog hands it over as a shared reference

**Question.** Upstream's `CalcValue` carries a unit, so `10kg + 500g` answers
`10,500 g`. YAL's value carried an amount, a percentage flag, a boolean flag
and a currency, and no unit at all. How does a value carry one?

**Options.** (a) The unit by name, and the scale and the dimension looked up
whenever they are needed. (b) The unit as a shared reference into the
catalog's own table, with the scale and the dimension carried along.

**Decision.** (b). A measurement holds the symbol, the long name, the
category, the scale onto the base, the offset and the dimension, and it is
`Copy`, so a calculation carries it without an allocation on every step. The
catalog hands it over through one new method, `unit_ref`, which answers a
shared reference into the catalog's table; a catalog that knows the names but
not the scales answers `None`, and the core carries no unit rather than a
guessed one.

**Why.** A calculation reads the scale and the dimension of a unit many times
- once for each operation that touches it - and it compares them for
compatibility. Carrying the numbers along is cheaper and, more importantly,
makes the comparison impossible to get wrong: two units are compatible when
their dimensions are equal, not when their names look alike.

**What this does not change.** The conversions between two plain units, which
were already ported and are unaffected; and the currency, which stays a
separate kind of value.

## 2026-11-13 - A compound unit keeps one copy of its symbol

**Question.** Upstream builds the symbol of a compound unit by joining the
symbols of its two parts: `kg` over `m³` is `kg/m³`, and `kg` times `m` is
`kg·m`. This port has to keep that symbol in a value that is `Copy`. How?

**Options.** (a) An owned `String` in the value, and the value is no longer
`Copy`. (b) The parts of the compound, rendered on demand. (c) One copy of
every symbol that has been met, kept for the life of the process.

**Decision.** (c). The symbol of a compound is interned: the registry in
`tc-calc::measure` keeps one copy of each distinct symbol and hands out a
shared reference to it.

**Why.** The symbol is not a decoration. `kg/m³` and `kg·m` are different
measures, and the way the two parts were joined is part of what the measure
is. Rendering the parts on demand cannot always recover that: `kg` over
`m/s` is written `kg/(m/s)`, and a flat list of parts cannot tell it from
`kg/m·s`. Interning keeps the symbol exactly as upstream writes it, keeps
the value `Copy`, and the number of distinct symbols a calculator meets is
small.

**What this does not change.** The unit table itself, which is still
transcribed by script and never typed by hand.

## 2026-11-13 - The rows whose documentation lost a multiplication sign

**Question.** The unit section of the documentation writes `5m 4m` and expects
`20 m²`. YAL answers `9 m`, because two quantities written side by side are
one amount. Upstream's own tests write `5m * 4m`. What do the ledger rows say?

**Options.** (a) Mark the rows `NOT PORTED`, because the query as written does
not answer as the row says. (b) Mark them `PORTED`, with the evidence of the
example as upstream's tests write it, and write down why.

**Decision.** (b). The rows are `PORTED` with the evidence of the example with
the multiplication sign, and `docs/port-notes/023` holds the whole of it: the
documentation's markdown reads `*` as emphasis and drops it, upstream's tests
write the sign out, and without the sign the two quantities are one amount -
which is upstream's behaviour too, and which this port answers the same way.

**Why.** The row describes a feature - measurements that combine into area,
volume and speed - and the feature is ported and tested. The text of the row
is a rendering artefact of the documentation, not a behaviour of upstream's
program. Marking it `NOT PORTED` would say that upstream's own behaviour is
missing, which is not true.

**What this does not change.** The rule that a row is `PORTED` only when the
cited evidence is a test that ran; the cited test holds both readings.

## 2026-11-14 - The budget the app has to live in

**Question.** The port exists because Raycast is comfortable and Raycast is
heavy. What does YAL cost, and what do we do about it?

**The measurement.** Counting whole process trees on this machine: Raycast
**914 MB** of working set (its own application, six WebView2 processes and two
screen-dimming helpers), YAL **367 MB** with the palette open. The Rust core
that holds everything this port wrote - calculator, clipboard, snippets, file
index - is **6 MB**.

**Options.** (a) Keep Tauri and WebView2 as they are. (b) Make the web view
lazy - create it when the palette opens, destroy it when it closes. (c) Render
the palette natively and drop the browser engine.

**Decision.** (c) is the destination; (b) is not the first move; (a) is
rejected. The budget is a release requirement either way: under 40 MB resident
and under 0.5% of one core idle, palette open or closed.

**Why.** The weight is in the UI runtime, not in the engine, so only the UI
layer can fix it. But the lazy web view pays for the saving on the one
interaction that matters: the palette would take a few hundred milliseconds to
open instead of tens, because a fresh WebView2 has to be built on the hotkey
path; the transient 400 MB spike would still appear every time the palette
opened; the first keystrokes could be dropped while the input method attaches;
and create-destroy on every toggle is where tray apps acquire the bugs that
make a hotkey stop answering. A launcher is opened dozens of times a day, so
that latency tax is permanent while the memory saving is only visible while
the app is idle. The native palette is ten times lighter than the current
build, more than twenty times lighter than Raycast, and it opens instantly.

**What this does not change.** The stack, yet. Tauri, Solid and the TypeScript
frontend stay until the palette is replaced by a native renderer, and no engine
crate is rewritten for this. It also does not change the parity ledger:
resource use is not a feature row, and no row moves because of this decision.

**Correction, 2026-11-15.** The first version of this entry compared YAL's
whole tree against only the processes named `Raycast*`, which reported 331 MB
for Raycast and made YAL look heavier. Raycast also owns a WebView2 and two
`Dimmer` helpers, so its real figure is 914 MB and YAL is the lighter of the
two. The measurement script now counts whole trees and stops at apps Raycast
happened to launch.
