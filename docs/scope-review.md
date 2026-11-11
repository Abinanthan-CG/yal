# YAL: what is still to build

Generated from parity/parity.toml on 2026-11-11 20:25.
Every open row is here, grouped into the feature it belongs to.

**How to use it.** Tick the features you want. A tick means: build it, with a test
that shows it works. A cross means: leave it out, and the row keeps its one-line reason
in parity/reasons.toml - a row that is not built is never silently claimed.

The count in brackets is how many ledger rows the feature covers.

**2189 rows are open: 1566 to build and 623 out of scope by decision.**

## At a glance

What was decided on 2026-11-11, and what each page becomes.

| Page | Rows | Decision |
| --- | --- | --- |
| Features > Calculator | 296 | **BUILD** - All of it, 1:1. The currencies come last: they need live rates. |
| Ai | 0 | **CUT** - No model provider, no API key. Decided 2026-11-11. |
| Launcher > Commands | 107 | **BUILD** - The built-in command catalogue, one row per command. |
| Reference > Shortcuts | 83 | **BUILD** - Which key does what, per screen. The AI screens went with the AI. |
| Features > Window Management | 78 | **BUILD** - Switching, moving and resizing windows - real Win32. |
| Reference > Settings | 76 | **BUILD** - The settings panes, their groups and their defaults. |
| Features > Calendar | 72 | **BUILD** - Windows' own calendar through WinRT, plus a command that opens the web calendar. |
| Features > Clipboard | 70 | **BUILD** - All of it, 1:1: the capture loop, the watcher and the section. |
| Features > File Search | 68 | **BUILD** - The indexer and the search, over NTFS. |
| Palette | 66 | **BUILD** - The app's own shell: the screens, the back stack, the actions menu, the keys. |
| Launcher | 64 | **BUILD** - The launcher's own behaviour. |
| Launcher > Quicklinks | 64 | **BUILD** - The store, the section and the editor. |
| Reference > Hotkeys | 63 | **BUILD** - The global shortcuts you record yourself. |
| Features > Notes | 60 | **BUILD** - A folder of Markdown files, as upstream. |
| Features > Snippets | 59 | **BUILD** - The section, the editor and the browser. |
| Ai > Mcp | 0 | **CUT** - With the AI. |
| Reference > Backup | 49 | **BUILD** - The export and the import of your data. |
| Permissions | 0 | **CUT** - The macOS permissions have no Windows answer. |
| Ai > Quick Actions | 0 | **CUT** - With the AI. |
| Reference > Deeplinks | 45 | **BUILD** - Our own tinycast:// grammar. |
| Features > Dictation | 0 | **CUT** - Windows' own speech input does the job. |
| Reference > Import From Raycast | 41 | **BUILD** - Low priority: the .rayconfig export can be done by hand. |
| Features > Window Layouts | 0 | **CUT** - The saved layouts, and the rooms that hold them, went with it. |
| Features > Navigation | 33 | **BUILD** - Switching windows stays; reading a menu bar did not. |
| Launcher > Uninstall | 0 | **CUT** - The in-app uninstaller. |
| The Overview | 30 | **BUILD** - The first-run story: pick a shortcut, turn on what you want. |
| Launcher > System Actions | 28 | **BUILD** - The twenty that run stay; the twelve that cannot stay open with their reason. |
| Features > Rooms | 0 | **CUT** - With the window layouts. |
| Extensions > Installing | 0 | **CUT** - A macOS concept. |
| Extensions | 0 | **CUT** - A macOS concept. |
| Reference > Updates | 0 | **CUT** - One person rebuilds it from the source. |
| Features > Emoji | 25 | **BUILD** - The picker screen; the model is already done. |
| Features > Apple Shortcuts | 0 | **CUT** - A macOS concept. |
| Install | 21 | **BUILD** - Already done. |
| Extensions > Compatibility | 0 | **CUT** - A macOS concept. |
| Extensions > Customising | 0 | **CUT** - A macOS concept. |
| Launcher > Favorites | 19 | **BUILD** - The favorites slots and the section. |
| Launcher > Aliases | 18 | **BUILD** - The alias store. |
| Launcher > System Settings | 16 | **BUILD** - The Windows Settings pages, opened straight from the launcher. |
| Launcher > Fallbacks | 15 | **BUILD** - The fallbacks. |
| Features > Camera | 0 | **CUT** - Not for now. |
| Features > Dictionary | 0 | **CUT** - Define Word opens a web lookup instead. |

## To build

## Features > Calculator

_296 rows to build._

- [ ] **Arithmetic** (13)
    - Math, units, live currency and crypto rates, dates and time zones, answered as you type.
    - Type a calculation into the launcher and the answer appears on a card above the results. There's no separate calculator mode; Tinycast works out th...
    - Copy Answer — return.
    - _...and 10 more in this feature._
- [ ] **Calculator History** (6)
    - Calculator History is a separate screen that you open with the command of the same name. It isn't part of the tab cycle. To leave it, press esc, or...
    - Copy Answer — return.
    - Copy Expression, on a past entry — ⌘return.
    - _...and 3 more in this feature._
- [ ] **Colors** (1)
    - Paste a color like #FF5733, rgb(255, 87, 51) or hsl(11, 100%, 60%) into the launcher and a card shows the color. ⌘K lets you copy it as hex, rgba()...
- [ ] **Cooking** (5)
    - 1 cup of flour in grams — 125 g.
    - 200 g butter in cups — 0.8810572687 cup.
    - 2 tbsp honey to g — 42.375 g.
    - _...and 2 more in this feature._
- [ ] **Currency And Crypto** (13)
    - 1 euro to dollars — Named currencies.
    - €20 to GBP / 20€ to GBP — Symbols, on either side.
    - Eur to usd — An amount of 1.
    - _...and 10 more in this feature._
- [ ] **Dates And Times** (35)
    - Time until a moment — hrs till 9am, days till 9april.
    - Time since a moment — days since 9jul, hrs since noon.
    - A moment plus or minus — today + 3 weeks, now + 90 min.
    - _...and 32 more in this feature._
- [ ] **Electrical** (8)
    - 5 watt 3h 30min to kwh — 0.0175 kWh.
    - 12V 2A — 24 W.
    - 12V / 6ohm — 2 A.
    - _...and 5 more in this feature._
- [ ] **Errors** (1)
    - The calculator only shows an error for real mistakes, like combining two units that don't fit together (1kg + 1m) or a unit and a currency (Cannot ...
- [ ] **Fuel And Pace** (7)
    - Fuel economy uses mpg and km/L, and fuel consumption uses L/100km. 15 km/l to mpg is 35.2821875 mpg, and 100 mi / 4 gal to mpg is 25 mpg. Economy a...
    - 30 mpg to l/100km — 7.840486111 L/100km.
    - 6.5 l/100km to mpg — 36.18685897 mpg.
    - _...and 4 more in this feature._
- [ ] **Functions And Comparisons** (3)
    - Min, max, sum, avg, mean, average, median and stddev take lists, including measurements, and count counts them: sum(1km,500m) is 1.5 km. Inside a f...
    - ==, !=, and >= compare numbers or compatible units: 1km == 1000m is true. Whole numbers support &, |, xor, ~, >. ^ always means a power.
    - Base conversion works both ways: 0xff shows Hexadecimal → Decimal, 256 as hex is 0x100, and 2m / 2m to hex is 0x1.
- [ ] **Growth And Decay** (5)
    - 20 after 5 years at 5% every year — 25.52563125.
    - 20 after 5 years at 5% decay every year — 15.47561875.
    - Time from 20 to 30 at 5% every year — 8.310386223 yr.
    - _...and 2 more in this feature._
- [ ] **Holidays** (11)
    - Easter and its days — easter 2027, good friday 2027 — 28 March, 2027.
    - Orthodox Easter — orthodox easter 2027 — 2 May, 2027.
    - Chinese New Year — chinese new year 2027 — 6 February, 2027.
    - _...and 8 more in this feature._
- [ ] **Interest And Loans** (12)
    - $2,000 after 5 years at 4% — 2,433.31 USD.
    - $2,000 at 4% for 5 years compounding quarterly — 2,440.38 USD.
    - Interest on $2,000 after 5 years at 4% — 433.31 USD.
    - _...and 9 more in this feature._
- [ ] **Lists And Statistics** (12)
    - Average of 10, 20, 30 — 20.
    - Median of 3, 1, 2 — 2.
    - Count of 4, 8, 15, 16, 23, 42 — 6.
    - _...and 9 more in this feature._
- [ ] **Math In Words** (14)
    - Is 97 prime — true.
    - Is 91 a prime number — false.
    - Remainder of 20 divided by 3 — 2.
    - _...and 11 more in this feature._
- [ ] **Music** (1)
    - 440 hz to pitch is A4, the nearest note with A4 at 440 Hz, and c4 to hz is 261.6255653 Hz. Write notes as a letter, an optional b for flat, and an ...
- [ ] **Number Format** (2)
    - Settings → General → Calculator → Number format sets how you write numbers and how answers are shown. System (the default) follows your Mac's Langu...
    - With a decimal comma, separate function values and list items with ;. If a number could be read two ways, the calculator shows no card rather than ...
- [ ] **Other Formats** (6)
    - 1234567 to sci — 1.234567e6.
    - 0.000123 to scientific notation — 1.23e-4.
    - 51.4772° to dms — 51° 28′ 37.92″.
    - _...and 3 more in this feature._
- [ ] **Percent Ratios And Lists** (30)
    - 20% off 500 — 400.
    - 25% off $60 — 45.00 USD.
    - Take 20% off 100 — 80.
    - _...and 27 more in this feature._
- [ ] **Pixels And Density** (8)
    - 3000px / 300ppi to inches — 10 in.
    - 5in 300ppi — 1,500 px.
    - 3000px / 10in to ppi — 300 ppi.
    - _...and 5 more in this feature._
- [ ] **Rates** (5)
    - Rates are saved on your Mac and refreshed every 24 hours, counted from when they were saved. Restarting Tinycast doesn't refresh them early, so if ...
    - A money answer shows when the rates were last updated. Click the refresh button next to it to load new rates now.
    - Crypto prices are best effort. If they fail to load, currencies still work, and Tinycast tries loading crypto prices again in 30 minutes.
    - _...and 2 more in this feature._
- [ ] **Rates Downloads And Playback Speed** (10)
    - $50/hr for 3 hours — 150.00 USD.
    - 60 km/h for 30 min — 30 km.
    - $100,000/year for 3 months — 25,000.00 USD.
    - _...and 7 more in this feature._
- [ ] **Rem And Em** (5)
    - 24px — 1.5 rem.
    - 2rem — 32 px.
    - 16px to rem — 1 rem.
    - _...and 2 more in this feature._
- [ ] **Rounding And Fractions** (1)
    - $1,240 rounded to nearest hundred — 1,200.00 USD.
- [ ] **Splitting A Bill** (7)
    - $120 split 4 — 30.00 USD.
    - Split 120 between 4 — 30.
    - 85 + 20% split 3 — 34.
    - _...and 4 more in this feature._
- [ ] **Time Zones** (21)
    - Time in Tokyo — The current time there.
    - What time is it in London — The same.
    - 5pm ldn in sf — 5 PM London time, in San Francisco.
    - _...and 18 more in this feature._
- [ ] **Units** (40)
    - The last unit you type sets the unit of the answer.
    - 5feet + 1m — 2.524 m.
    - 10kg + 500g — 10,500 g.
    - _...and 37 more in this feature._
- [ ] **Unix Time** (1)
    - Now to unix, current timestamp and new timestamp give whole seconds, and now to unix ms gives milliseconds. now to iso gives an ISO 8601 date like ...
- [ ] **Volume And Flow** (7)
    - 1m3 — 1,000 L.
    - 2m 30cm 40cm to l — 240 L.
    - Pi (10cm)^2 30cm to l — 9.424777961 L (cylinder).
    - _...and 4 more in this feature._
- [ ] **Words With Two Meanings** (3)
    - Some currency words are shared by several countries: dollars could mean 22 currencies, francs 10, pounds 9, pesos 8 and rupees 6. Tinycast maps eac...
    - Slang isn't supported: quid and bucks show no card. rmb and renminbi work, because the ISO 4217 standard names the currency \"Yuan Renminbi\".
    - Units take priority over currencies, so 10 pounds to kg is a weight, 10 pounds to euros is money, and 1 cup to ml is a volume even though CUP is al...
- [ ] **Workdays** (3)
    - Weekdays, business days, work days and working days skip Saturdays and Sundays, as in today + 5 business days or 5 weekdays from now. Public holida...
    - To count workdays between two dates, use workdays between july 27 and august 7 or august 3 to august 14 in workdays.
    - As a unit on its own, a workday is eight hours: 55h in workdays.

## Launcher > Commands

_107 rows to build._

- [ ] **Arguments** (8)
    - Select a command with arguments and small fields appear beside the search field. The command stays selected while you fill them in.
    - <kbd>tab</kbd> moves through the fields; <kbd>⇧</kbd><kbd>tab</kbd> moves back.
    - <kbd>return</kbd> runs the command once every required value is present. Otherwise it focuses the
    - _...and 5 more in this feature._
- [ ] **Built In Commands** (45)
    - Every built-in command, and the shell commands you write yourself.
    - Commands are actions you can run by name from the launcher. Some are built in, and you can add your own shell commands.
    - Each built-in command belongs to one Settings pane. A feature's commands are in that feature's pane and only appear while the feature is on. The re...
    - _...and 42 more in this feature._
- [ ] **Custom Commands** (8)
    - Give a shell command a name, then run it from search or with its own global shortcut.
    - Turn custom commands on in Settings → Commands → Custom Commands. They're off by default.
    - Enable custom commands — Off.
    - _...and 5 more in this feature._
- [ ] **Editing And Backups** (2)
    - Editing a command keeps its favorite, visibility and shortcut. Deleting it removes all three.
    - Custom commands and their shortcuts are included in backups. When you import a backup, Tinycast warns you before adding any commands, so a backup c...
- [ ] **How A Command Runs** (6)
    - Shell — /bin/zsh -lc.
    - Folder — Its Run In folder, or your home folder.
    - Input — None; anything that reads input gets an end-of-file.
    - _...and 3 more in this feature._
- [ ] **Importing Raycast Script Commands** (8)
    - Settings → Commands → Import Raycast Scripts reads a folder of Raycast script commands and adds a custom command for each script. It's a one-time i...
    - A file counts as a script command when it has both a shebang line and an @raycast.title. Other helper scripts in the folder are skipped.
    - @raycast.title — The name.
    - _...and 5 more in this feature._
- [ ] **Load Shell Environment** (9)
    - This setting is off by default, and it's the most common reason a command fails.
    - Zsh only reads ~/.zshrc for interactive shells. The default -lc reads .zprofile and .zlogin, but not .zshrc. As a result, your aliases, functions a...
    - Turning the setting on uses -ilc, which reads .zshrc. That also runs everything else in your shell startup, like oh-my-zsh's auto-update or a theme...
    - _...and 6 more in this feature._
- [ ] **Needs Confirmation** (2)
    - This is on by default for every command. The palette closes, and a dialog shows the command's name and full text. return runs it and esc cancels.
    - The dialog is a plain confirmation rather than a red warning, since you wrote the command yourself. You can't skip it from the palette or a shortcu...
- [ ] **Show Output** (4)
    - With Show output on, a window opens as soon as the command starts and shows its output as it's printed, in order and in color.
    - The window shows the command's name and text, the output, the result and how long it took. Copy is always available. Stop ends the command and ever...
    - The window keeps the last 256 KB of output. If you scroll up, it stops following new output. Scroll back to the bottom and it follows again.
    - _...and 1 more in this feature._
- [ ] **The Editor** (10)
    - Name — What you search for.
    - Command — The shell command, like /usr/bin/pmset displaysleepnow.
    - Icon — A symbol for its row, dialogs and output window.
    - _...and 7 more in this feature._
- [ ] **When It Finishes** (5)
    - **Success, with Show confirmation on:** a short message shows the last line of output, or
    - \"Ran \" if the command printed nothing.
    - **Failure:** a dialog shows the error output, up to the last 8 KB.
    - _...and 2 more in this feature._

## Reference > Shortcuts

_83 rows to build._

- [ ] **Calculator History** (5)
    - Return — Copy Answer.
    - ⌘return — Copy Expression, on a past entry.
    - ⌘return — Put Answer in Search Bar, on a new calculation.
    - _...and 2 more in this feature._
- [ ] **Clipboard** (11)
    - Return — Paste.
    - ⌘return — Copy to Clipboard.
    - ⌥return — Paste and Keep Window Open.
    - _...and 8 more in this feature._
- [ ] **Custom Commands** (5)
    - Return — Run Command.
    - ⌘N — Create Custom Command.
    - ⌘E — Edit Custom Command.
    - _...and 2 more in this feature._
- [ ] **Dialogs** (1)
    - On every dialog, including ones that delete things, return clicks the main button and esc cancels. Cancel is always on the left.
- [ ] **Emoji** (4)
    - Return — Paste.
    - ⌘return — Copy to Clipboard.
    - ⌥return — Paste and Keep Window Open.
    - _...and 1 more in this feature._
- [ ] **Everywhere In The Palette** (15)
    - Every built-in shortcut in the palette and its screens, in one place.
    - These shortcuts are built in and can't be changed. For shortcuts you record yourself, see Hotkeys.
    - Letter shortcuts follow your ASCII keyboard layout even while a non-Latin input source is active. Dvorak and other ASCII layouts keep their own let...
    - _...and 12 more in this feature._
- [ ] **File Search** (9)
    - Return — Open File / Open Folder.
    - ⌘return — Show in Finder.
    - ⌘P — Filter by type.
    - _...and 6 more in this feature._
- [ ] **Launcher** (11)
    - Return — Open.
    - ⌘return — Show in Finder (app, setting, snippet).
    - ⇧⌘F — Add to / Remove from Favorites.
    - _...and 8 more in this feature._
- [ ] **Notes Window** (6)
    - ⌘N — Create a note.
    - ⌘P — Open the switcher.
    - ⌘O — Open the Notes folder.
    - _...and 3 more in this feature._
- [ ] **Quicklinks** (7)
    - Return — Open Quicklink.
    - ⌘return — Open With Default App.
    - ⌘E — Edit Quicklink.
    - _...and 4 more in this feature._
- [ ] **Snippets** (4)
    - Return — Paste Snippet.
    - ⌘return — Show in Finder.
    - ⌘E — Edit Snippet.
    - _...and 1 more in this feature._
- [ ] **Switch Windows** (5)
    - Return — Switch to Window.
    - ⌃⇧M — Minimize Window.
    - ⌃⇧H — Hide Application.
    - _...and 2 more in this feature._

## Features > Window Management

_78 rows to build._

- [ ] **Custom Sizes** (3)
    - When no built-in size suits your screen, make your own. In Settings → Window Management → Custom Sizes, click New Custom Size and give it a name, a...
    - A custom size works like a built-in command. It has its own launcher entry, shortcut, alias and checkbox, and it follows Show in launcher. It resiz...
    - Restore Window undoes a custom size. Custom sizes never cycle.
- [ ] **Cycling And Restore** (8)
    - Cycling only applies to the four halves and Center Half, and it's off by default. With cycling off, pressing Left Half again leaves the window wher...
    - **Cycle ½, ⅓ and ⅔** changes the width with each press, keeping the same side. Top Half and Bottom
    - Half cycle through heights instead. Center Half stays centered as its width changes.
    - _...and 5 more in this feature._
- [ ] **Fullscreen** (3)
    - Toggle Fullscreen first asks the window to enter fullscreen, then tries clicking its green button. If neither works, it does nothing.
    - It never sends ⌃⌘F, because apps can use that shortcut for something else, and triggering the wrong command would be worse than doing nothing.
    - It resets the cycle but keeps the Restore point.
- [ ] **How Sizes Are Calculated** (5)
    - Gaps. Edges next to the screen border get the full gap, and edges between two windows get half, so two windows side by side have exactly one gap be...
    - Make Larger and Make Smaller change the size by 5% of the screen, not the window, so each one exactly undoes the other. The largest size is the ful...
    - Reasonable Size is 60% of the screen, centered, and never larger than 1025 × 900 points, so on a large 5K display you get a comfortably sized windo...
    - _...and 2 more in this feature._
- [ ] **Settings** (10)
    - Enable window management — On.
    - Enable window management — Off — Off.
    - Show in launcher — On.
    - _...and 7 more in this feature._
- [ ] **Spaces** (3)
    - Switch to Previous Space and Switch to Next Space switch between macOS Spaces without the sliding animation. A switch takes about 56 ms, compared w...
    - They send the same trackpad swipe macOS uses to switch Spaces, fast enough that the switch completes without animating. Both desktop Spaces and ful...
    - If you hold down the shortcut, presses that arrive during a switch are ignored, so a long press never skips too far. At the first or last Space, ma...
- [ ] **The Commands** (44)
    - 39 commands for halves, quarters, thirds, sizing, nudges, edges, displays and fast Space switching.
    - Move and resize the window you were last using, without installing another app.
    - It doesn't need any new permissions. It uses the same Accessibility permission as clipboard pasting.
    - _...and 41 more in this feature._
- [ ] **When A Window Won T Move** (2)
    - Minimized windows, sheets, popovers, windows already in native fullscreen, and windows that don't report a position or size are skipped.
    - A window that can move but can't be resized keeps its size and moves to the command's position. A window that can't move is left untouched. If an a...

## Reference > Settings

_76 rows to build._

- [ ] **Advanced** (2)
    - Backup. Export a backup, import one, or import from Raycast. See Backup & restore. Sync settings file (Off) keeps your settings in a JSON file you ...
    - About. Version, license, Check for Updates, links to the project, and Support. See Updates.
- [ ] **Features** (13)
    - Everything here is off by default, except Clipboard and Emoji & Symbols.
    - Clipboard — Enable Clipboard History (On) — Clipboard commands, Keep history for (3 Months), Search text in images and PDFs (Off), Default action (...
    - Snippets — Enable snippets — Show in launcher, snippet commands, New Snippet, Snippets Folder.
    - _...and 10 more in this feature._
- [ ] **General** (4)
    - Every Settings pane, what's in it, and the defaults.
    - Open Settings with ⌘, from the palette, the Tinycast Settings command or the menu bar icon. It's a regular, resizable window.
    - The search field finds any setting by name, and also by related words that aren't in its title: ocr finds Search text in images and PDFs, and caps ...
    - _...and 1 more in this feature._
- [ ] **General 2** (47)
    - Show Launcher — None.
    - Language — English, Deutsch, Español, Français, 日本語, 한국어, Português, Русский, 简体中文 — Your Mac's language if Tinycast has it, otherwise English.
    - Launch at login — On.
    - _...and 44 more in this feature._
- [ ] **Launcher** (9)
    - Applications — Search Scopes, Enable Applications, and a row per app.
    - System Settings — Enable System Settings, and a row per pane.
    - System Actions — Enable System Actions, and a row per action.
    - _...and 6 more in this feature._
- [ ] **Permissions** (1)
    - Shows whether Accessibility, Full Disk Access, Calendars and Microphone access is granted, and opens the right System Settings pane. See Permissions.

## Features > Calendar

_72 rows to build._

- [ ] **Auto Join** (6)
    - Auto Join Meetings (off by default) opens each meeting when it starts.
    - It only joins meetings that start **after** you turned it on.
    - It joins each meeting **once**, even if you decline the confirmation.
    - _...and 3 more in this feature._
- [ ] **Backups** (1)
    - Three settings are never included in backups, because each one is a form of consent: the calendar switch itself, Auto Join Meetings and Camera Prev...
- [ ] **Camera Preview** (2)
    - Camera Preview (off by default) shows your camera before you join, so you can check how you look and what's behind you. return joins and esc cancels.
    - The preview replaces the confirmation, so you're never asked twice. The camera only turns on when the preview opens and turns off as soon as it clo...
- [ ] **Choosing Calendars** (1)
    - The Calendars list at the bottom of the pane has a search field and a checkbox for each calendar. Holidays and Birthdays are the ones people usuall...
- [ ] **Commands** (6)
    - Join Next Meeting — Opens the card's meeting, the running one, or the next one.
    - My Schedule — Shows your meetings as a list in the palette.
    - Create Event — Opens a form for a title, start time and duration, then adds the event to your default calendar.
    - _...and 3 more in this feature._
- [ ] **Four Ways To Join** (12)
    - Join your next meeting with one key, see your day, and show the next event in the menu bar.
    - Tinycast reads the calendars already on your Mac, finds the join link in each meeting, and lets you join with one keystroke. It works with any cale...
    - Turn it on in Settings → Calendar → Join meetings from Tinycast. It's off by default. When you turn it on, Tinycast explains what it reads, and the...
    - _...and 9 more in this feature._
- [ ] **How Far Ahead It Reads** (1)
    - Days to Show sets how far ahead Tinycast reads: Today, Today and Tomorrow (default) or Next 7 Days. My Schedule, the menu bar item and launcher sea...
- [ ] **Meeting Details** (1)
    - Show Details opens a page with the meeting's day, time and calendar, its location, the attendees and their replies, and the description. return, ⌘r...
- [ ] **Meetings In Search** (1)
    - Upcoming meetings also appear in launcher search, in their own Meetings section. Upcoming meetings in launcher sets how many: 1 next, 3 next, 5 nex...
- [ ] **My Schedule** (1)
    - My Schedule lists your remaining events for the days Tinycast reads, grouped by day. Type to filter. return joins the selected meeting, and ⌘K offe...
- [ ] **The Join Card** (7)
    - The card appears a set time before a meeting starts and stays for the same amount of time after, but never past the meeting's end. Show the meeting...
    - Join Meeting — return.
    - Copy Meeting Link — ⌘return.
    - _...and 4 more in this feature._
- [ ] **The Menu Bar** (23)
    - The calendar has its own menu bar item, separate from the Tinycast icon. You can show either one, both or neither.
    - Calendar in Menu Bar — Disabled.
    - Calendar in Menu Bar — Meeting Icon.
    - _...and 20 more in this feature._
- [ ] **Which Links It Finds** (10)
    - Tinycast checks the event's URL, location and notes, in that order. It recognizes Zoom, Google Meet, Microsoft Teams, Webex, Jitsi, Whereby, Amazon...
    - **A link to a known service always takes priority over a plain link**, so a \"reset your password\"
    - Link at the top of an invite never wins over the Meet link below it.
    - _...and 7 more in this feature._

## Features > Clipboard

_70 rows to build._

- [ ] **Actions** (18)
    - Paste — return.
    - Copy to Clipboard — ⌘return.
    - Paste as Plain Text — ⌃⌘return.
    - _...and 15 more in this feature._
- [ ] **Filtering By Type** (2)
    - ⌘P opens a filter to the right of the search field. There are seven options, and every entry belongs to exactly one:
    - Each filter has its own empty message, so \"Clipboard history is empty\" never appears when the history only looks empty because of a filter. The f...
- [ ] **Limits Worth Knowing** (7)
    - History is stored in clipboard.sqlite3, with images next to it, in Tinycast's Application Support folder. Because it isn't in Caches, Time Machine ...
    - Tinycast checks the clipboard twice a second. It recognizes and skips its own pastes, so pasting
    - From Tinycast never adds a new entry.
    - _...and 4 more in this feature._
- [ ] **Opening It** (7)
    - Search the text, images, files and colors you copied, and paste them back where you were.
    - Tinycast saves what you copy, so you can find it later and paste it back into the app you were using.
    - Clipboard history is the only feature that's on by default. To turn it off, go to Settings → Clipboard → Enable Clipboard History. When it's off, n...
    - _...and 4 more in this feature._
- [ ] **Paste Sequentially** (1)
    - Give Paste Sequentially a shortcut in Settings → Clipboard to paste a series of copies one at a time without opening the palette. If you copy A, th...
- [ ] **Pinning** (3)
    - ⌘. pins the selected entry.
    - First ten visible pinned entries.
    - Entries keep them too. To remove a pinned entry, use Delete Entry.
- [ ] **Searching Text Inside Images And Pdfs** (9)
    - Search text in images and PDFs is off by default. When you turn it on, Tinycast reads the text in your copied images, image files and PDFs, so sear...
    - Text recognition happens **on your Mac**, one item at a time, while you aren't typing or moving
    - The mouse.
    - _...and 6 more in this feature._
- [ ] **Settings** (18)
    - Settings → Clipboard.
    - Enable Clipboard History — On.
    - Enable Clipboard History — Off — On.
    - _...and 15 more in this feature._
- [ ] **What It Keeps** (5)
    - **Images**, like screenshots, saved as PNG files. When you paste an image, a terminal receives the
    - Path of its saved file, because terminals ignore image data.
    - **Files you copy in Finder.** Tinycast saves a reference to the file in its current location, not
    - _...and 2 more in this feature._

## Features > File Search

_68 rows to build._

- [ ] **Actions** (20)
    - Open File / Open Folder — return.
    - Show in Finder — ⌘return.
    - Share…
    - _...and 17 more in this feature._
- [ ] **Filtering By Type** (2)
    - ⌘P, or the All Types button, narrows the search:
    - The filter is applied during the search, so it never hides matches that were already cut off by the result limit. It resets each time you open the ...
- [ ] **Full Disk Access** (14)
    - Find files and folders instantly from a private index on your Mac. Needs Full Disk Access.
    - Search file and folder names in the folders you choose. Tinycast builds its own index of those folders and keeps it current as files change, so res...
    - Turn it on in Settings → File Search. It's off by default. While it's off, there's no command and no index on disk.
    - _...and 11 more in this feature._
- [ ] **How Search Works** (2)
    - Every word you type must appear in the file name, in any order. annual report finds \"Report – annual 2025.pdf\". Matching ignores case and accents.
    - The best matches come first: an exact name, then names that start with what you typed. Files in your home folder rank above hidden files, ~/Library...
- [ ] **Ignore Patterns** (12)
    - Set these in Settings → File Search → Ignore Patterns. A matching folder, and everything in it, is skipped while indexing. There are three kinds:
    - Plain word — Any folder or name — nodemodules.
    - With ? ` — Any folder or name — .tmp.
    - _...and 9 more in this feature._
- [ ] **In The Launcher** (3)
    - Turn on Show in launcher, under the File Search switch, to see files in launcher searches too. It's off by default.
    - When your search has three or more characters, a Files section lists up to five matches. It sits below the apps and commands, so return still opens...
    - The rows have the same actions as this screen. To see every match, choose the Search Files row under \"Use … with\".
- [ ] **Messages You Might See** (6)
    - Full Disk Access required — Tinycast can't index yet. Press return to open the settings pane.
    - Indexing your disk… — The first build is running. Results appear when it finishes.
    - Couldn't build the file index — The build failed. Try Rebuild Index in Settings.
    - _...and 3 more in this feature._
- [ ] **Search Scopes** (4)
    - Set these in Settings → File Search → Search Scopes. Only the folders listed here are indexed. By default, that's your home folder.
    - Everything inside a scope is indexed, including hidden files and the contents of app bundles. Those rank below your own files, and ignore patterns ...
    - An empty scope list indexes nothing. Tinycast doesn't fall back to your home folder. A folder inside another scope is already covered, so you don't...
    - _...and 1 more in this feature._
- [ ] **The Index** (2)
    - Settings → File Search → Indexed Items shows how many items are indexed and when the index was built. Rebuild Index builds it again from scratch.
    - The first build reads every folder in your scopes once. After that, Tinycast watches those folders and updates only what changed. The build runs in...
- [ ] **The Screen** (3)
    - With nothing typed, the list shows Recent: the 20 files in your home folder that changed most recently. Hidden files, ~/Library and app bundles are...
    - When you type, the list changes to Results. Each row shows the file's icon and name. Folders also show their parent folder's name, dimmed, which he...
    - The right side previews the selected file: the file itself, followed by its name, location, type, size and dates. You can play video and audio ther...

## Palette

_66 rows to build._

- [ ] **Appearance** (17)
    - Settings → General → Appearance has these settings:
    - Appearance — System.
    - Appearance — Light.
    - _...and 14 more in this feature._
- [ ] **Emacs Chords** (7)
    - These work anywhere in the palette:
    - ⌃N — ↓.
    - ⌃P — ↑.
    - _...and 4 more in this feature._
- [ ] **How Screens Stack** (7)
    - If you open a screen by typing its name in the launcher, it opens on top of the launcher, and esc takes you back to your search.
    - If you open a screen with its own global shortcut while the palette is hidden, it opens by itself with nothing behind it. If the palette is already...
    - Settings → General → Escape Key Behavior sets what esc does when the search is empty:
    - _...and 4 more in this feature._
- [ ] **Input Source** (2)
    - Settings → General → Auto-switch input source picks a keyboard layout to use while the palette is open.
    - This helps if you type Japanese or Chinese most of the day but your app names use Latin letters. Tinycast switches layouts when the palette opens a...
- [ ] **Moving Around** (15)
    - Move around Tinycast's floating palette and adjust its appearance and behavior.
    - The launcher, search tools and Quick AI share one floating panel. AI Chat, Notes, Camera and command output also have their own windows. The launch...
    - Return — The main action for the selection.
    - _...and 12 more in this feature._
- [ ] **Pop To Root** (3)
    - Settings → General → Pop to Root Search sets how long after the window closes Tinycast returns to the launcher: Immediately (default), or after 5, ...
    - Use a longer delay if you often close the palette and then come back to the same screen.
    - AI Chat has its own setting for reopening a conversation. See AI Chat.
- [ ] **Tab Moves Between Three Screens** (8)
    - Tab cycles through three screens: launcher → Quick AI → clipboard → launcher.
    - From the launcher, <kbd>tab</kbd> sends whatever you typed to [Quick AI](/docs/ai) as a question.
    - When this will happen, the header shows a Quick AI hint next to a tab key.
    - _...and 5 more in this feature._
- [ ] **The Menu Bar** (2)
    - The Tinycast menu bar icon has Open Tinycast, Clipboard History (while clipboard history is on), Check for Updates…, Support Tinycast…, Settings… a...
    - Settings → General → Show in menu bar hides the icon. Your shortcuts still work without it.
- [ ] **Where It Opens** (5)
    - Both placement settings are in Settings → General → Appearance.
    - Follow the cursor across displays (on by default) opens the palette on the display your pointer is on. Turn it off to always use the display with t...
    - Drag to move the launcher (off by default) lets you move the panel. Drag the thin strip above the search field, the empty space in the header, or t...
    - _...and 2 more in this feature._

## Launcher

_64 rows to build._

- [ ] **Actions On An App** (14)
    - Press ⌘K with an app selected:
    - Open Application — return.
    - Show in Finder — ⌘return.
    - _...and 11 more in this feature._
- [ ] **Hiding A Result** (3)
    - ⇧⌘H (Hide from Search) removes the selected entry from search results. The palette stays open on the same search.
    - It works for apps, System Settings panes, commands, Quick Actions, Apple Shortcuts, system actions, window commands, window layouts, rooms and exte...
    - Hiding only changes what search shows. The app stays installed, and its favorite, alias, learned ranking and shortcut keep working.
- [ ] **How Matching Works** (5)
    - The extension a command comes from — lucide for Lucide's Search Icons.
    - The bundle identifier or program name — apple.Photos.
    - A few more rules keep results useful:
    - _...and 2 more in this feature._
- [ ] **Learned Ranking** (3)
    - Opening something with its own global shortcut doesn't count, and neither do fallbacks or Open in Browser. Opening a favorite, including with ⌘ and...
    - Resetting. For one entry, choose ⌘K → Reset Ranking. It only appears when Tinycast has learned something about that entry. To reset everything, go ...
    - This data is stored in launcher-ranking.json in Tinycast's own folder and is never sent anywhere.
- [ ] **Listing A Whole Category** (2)
    - Type a section's name, like Snippets, Snippet or Window Management, to list that whole category under its own heading. An app with exactly that nam...
    - From three characters, the start of a section's name, or of a word in it, also works: win includes Window Management and Window Layouts. Your best ...
- [ ] **Multiple Installed Copies** (2)
    - If you have several versions of an app, each installed path appears in search. Version numbers, and the containing folder when needed, help you tel...
    - Copies share their alias, favorite, visibility, learned ranking and global shortcut. Favorites, Suggestions and Settings use one representative: th...
- [ ] **Names In Your Language** (3)
    - Apps are shown the way Finder shows them, in your Mac's language. The English name still works, so on a Mac set to Portuguese, both Find My and Bus...
    - Apps also match alternate names from their own metadata, such as Address Book for Contacts and System Preferences for System Settings. Localized na...
    - If you renamed an app in Finder, both the old and the new name find it.
- [ ] **Per App Shortcuts** (2)
    - You can give any app its own global shortcut in Settings → Applications. Press it to bring the app to the front, and press it again while the app i...
    - See Hotkeys for recording shortcuts and using double-tap shortcuts.
- [ ] **Search Scopes** (4)
    - Settings → Applications → Search Scopes sets which folders Tinycast searches for apps. A scope can be a folder or a single .app. Click Add… to choo...
    - The defaults cover /Applications, /System/Applications, both Utilities folders, /System/Library/CoreServices/Applications, the hidden system folder...
    - Tinycast searches one folder deep, so it finds /Applications/Blackmagic Design/DaVinci Resolve.app without a separate scope. Apps nested deeper nee...
    - _...and 1 more in this feature._
- [ ] **Suggestions** (6)
    - Suggestions shows up to five entries you're likely to want:
    - an app or extension you installed in the last five minutes and haven't opened yet
    - the entries you open most, unless they already have a shortcut
    - _...and 3 more in this feature._
- [ ] **Switching A Whole Section Off** (5)
    - Settings → Applications has an Enable Applications switch, plus a checkbox for each app.
    - Turning off **Enable Applications** removes every app from search **and** turns off every per-app
    - Shortcut.
    - _...and 2 more in this feature._
- [ ] **When You Type** (9)
    - The sections merge into one Results list, sorted by how well each entry matches. A few extras can appear around it:
    - **A calculator card** at the top when your text is a
    - Calculation, like 12% of 80 or time in Tokyo.
    - _...and 6 more in this feature._
- [ ] **With Nothing Typed** (6)
    - One search across everything Tinycast knows about, and how it orders the results.
    - The launcher is the root screen. One search covers your apps, System Settings panes, commands, quicklinks, Apple Shortcuts, snippets, system action...
    - Return opens the selection. ⌘K shows everything else you can do with it.
    - _...and 3 more in this feature._

## Launcher > Quicklinks

_64 rows to build._

- [ ] **As A Fallback** (1)
    - A quicklink with an {argument} also appears under \"Use … with\" at the bottom of every search. What you typed fills in its first argument. See Fal...
- [ ] **Destinations** (1)
    - Tinycast works out what kind of link you entered from its format:
- [ ] **Encoding** (1)
    - | raw turns encoding off. It also lets the value decide what kind of link it is: a {clipboard | raw} containing file:///Users/me/notes.md opens a f...
- [ ] **Filling In Values** (9)
    - When you select a quicklink that needs values, small fields appear in the search bar after what you typed. The quicklink's row stays visible the wh...
    - <kbd>tab</kbd> moves from the search field through each field and back.
    - <kbd>return</kbd> opens the quicklink. If a required field is empty, it moves to that field
    - _...and 6 more in this feature._
- [ ] **Import And Export** (4)
    - Import Quicklinks and Export Quicklinks use a simple JSON file you can edit by hand. A plain list also works, and only name and link are required.
    - The documentation shows this example:\n\n```\n{\n  \"version\": 1,\n  \"quicklinks\": [\n    {\n      \"name\": \"GitHub search\",\n      \"link\":...
    - A quicklink is skipped as a duplicate if its name or link matches one you already have, or one earlier in the same file. The summary shows how many...
    - _...and 1 more in this feature._
- [ ] **Making One** (17)
    - Turn a URL, search, file or deeplink into a command, with values you fill in when you open it.
    - A quicklink is a saved destination that works like any other launcher entry. You can search for it, give it a shortcut, and have it ask for a value...
    - Turn quicklinks on in Settings → Quicklinks. They're off by default.
    - _...and 14 more in this feature._
- [ ] **Opening** (3)
    - Open With saves a specific app. If you later uninstall that app, Tinycast shows a message with an Open with Default button instead of silently swit...
    - Open in a new window asks the browser to open a new window. Chrome and Firefox respect it, but Safari ignores it. With the setting off, the link op...
    - Tinycast checks file and folder links before opening them, so if a folder was deleted, it tells you.
- [ ] **Placeholders** (3)
    - Quicklinks use the same placeholders as snippets:
    - The documentation shows this example:\n\n```\nhttps://google.com/search?q={argument}\nhttps://github.com/search?q={argument name=\"Repository\"}\nh...
    - {cursor} and {snippet:…} are left as written in a link, because a URL has no cursor. Raycast's {query} works as {argument}.
- [ ] **Search Quicklinks** (14)
    - Quicklinks have their own launcher section. Only the name is searchable, along with any alias you give it. The link itself isn't.
    - Pinned quicklinks come first, in the order you pinned them, followed by the rest sorted by name. Pinning only affects the order within the Quicklin...
    - To change a quicklink from the launcher, select it and press ⌘E (Edit Quicklink).
    - _...and 11 more in this feature._
- [ ] **Selected Text From A Shortcut** (1)
    - Turn on Pass selected text as argument to fill a quicklink's first argument with the text you selected, when you open it with its own shortcut. Sel...
- [ ] **Three Ways To Hide A Quicklink** (8)
    - From broadest to narrowest:
    - Turning off **Show in launcher** in Settings removes all quicklinks and quicklink commands from
    - Search.
    - _...and 5 more in this feature._
- [ ] **When There Is No Selection** (1)
    - If a link uses {selection} and no text is selected, the quicklink doesn't open. Tinycast shows \"Couldn’t get the selected text\" instead. If Tinyc...
- [ ] **Where They Re Stored** (1)
    - Quicklinks are stored in quicklinks.sqlite3 in Tinycast's Application Support folder. If that file can't be opened, Tinycast reports the problem an...

## Reference > Hotkeys

_63 rows to build._

- [ ] **Caps Lock Details** (4)
    - While Caps Lock is your Hyper key, it's remapped at the hardware level on every keyboard, including one you plug in later. Any remapping another ap...
    - The remapping is removed when you choose a different key or quit Tinycast, and it never persists after a restart. If Tinycast is force quit or cras...
    - If a keyboard ever loses the remapping, one press there acts as plain Caps Lock. Tinycast then remaps that keyboard again and turns Caps Lock back ...
    - _...and 1 more in this feature._
- [ ] **Conflicts** (6)
    - **Hyperkey** or **Superkey** running: they have their own Hyper key, so Tinycast's stays off and
    - Says so. It starts again by itself when you quit them.
    - **BetterTouchTool** running: listed in Diagnostics as a possible conflict, nothing more.
    - _...and 3 more in this feature._
- [ ] **Dictation Key** (1)
    - Press 🎤 (F5) while recording any shortcut to use it in Tinycast, push-to-talk included. While a shortcut uses it, macOS Dictation doesn't; remove ...
- [ ] **Hiding Versus Turning Off** (4)
    - Hiding an item doesn't turn off its shortcut. Clearing a row's checkbox, or pressing ⇧⌘H in the launcher, only changes what search shows.
    - A section's switch turns off its shortcuts. Enable Applications, Enable System Settings, Enable System Actions and Enable Commands each turn off ev...
    - Turning off a feature turns off its shortcuts. File Search, Notes, AI, Quick Actions, Navigation, Calendar, window commands, quicklinks, snippets, ...
    - _...and 1 more in this feature._
- [ ] **Hyper Key** (4)
    - Settings → General → Hyper Key turns one physical key into ⌃⌥⌘, or into ⌃⌥⇧⌘ with Include Shift on.
    - You can choose Caps Lock, or the Left or Right side of Control, Shift, Option or Command.
    - Function keys aren't available. Their media functions run before Tinycast sees the key press, and catching them earlier would need Input Monitoring...
    - _...and 1 more in this feature._
- [ ] **Include Shift** (2)
    - Changing this setting updates every saved shortcut, so none of them stop working. If an update would conflict with another shortcut, Tinycast skips...
    - The option is dimmed while Hyper Key is set to None. Changing the physical Hyper key resets Quick Press to Does Nothing.
- [ ] **Keeping Them** (2)
    - Shortcuts are included in backups, except for custom Quick Actions and snippets.
    - Shortcuts for items you deleted while Tinycast wasn't running are removed the next time it starts.
- [ ] **Modifier Keys On Their Own** (16)
    - Instead of a key combination, a shortcut can be a single tap or a double tap of one ⌃, ⌥, ⇧, ⌘ or Globe (🌐︎) key. To record one, tap the key once ...
    - Tinycast remembers which side you pressed. A shortcut on Right ⌘ doesn't run when you tap Left ⌘. A single-tap shortcut shows a small L or R on its...
    - The rules:
    - _...and 13 more in this feature._
- [ ] **Quick Press** (2)
    - Settings → General → Quick Press sets what happens when you press Caps Lock by itself while it's your Hyper key: Does Nothing (default), Trigger Ca...
    - Many people who use Caps Lock as their Hyper key choose Escape.
- [ ] **Recording One** (4)
    - The recorder isn't a regular text field. Click it, then press the keys you want.
    - A small bubble above the field shows a prompt, then the modifiers you're holding. If the shortcut is already used by another Tinycast action, it re...
    - A key combination needs ⌘, ⌥, ⌃ or Globe (🌐︎). Only the function keys, F1 to F20, work without one. Press esc to stop recording, or delete to clea...
    - _...and 1 more in this feature._
- [ ] **Secure Input** (2)
    - Password fields, password managers and some terminals turn on macOS Secure Input, which hides every key press from apps like Tinycast. While it's o...
    - With Caps Lock as your Hyper key, turn on Secure Input Compatibility to keep Tinycast's own Hyper shortcuts working there. Caps Lock then acts as R...
- [ ] **Status And Diagnostics** (1)
    - The Status row under the key shows a dot: green while the Hyper key works. Click Diagnostics… to see what's running, which keyboards are remapped, ...
- [ ] **The Symbol** (2)
    - Any shortcut that includes all the Hyper modifiers is shown as a single ✦. For example, ⌃⌥⌘G is shown as ✦G, or ✦⇧G if it uses an extra modifier.
    - This only changes how the shortcut is displayed. With no Hyper key set, ⌃⌥⌘G is shown as it is.
- [ ] **What Can Have A Shortcut** (13)
    - Recording global shortcuts, modifier keys on their own, and the Hyper key.
    - Tinycast doesn't come with any shortcuts. You record every global shortcut yourself.
    - The palette itself (**Show Launcher**, in Settings → General)
    - _...and 10 more in this feature._

## Features > Notes

_60 rows to build._

- [ ] **Commands** (7)
    - Plain Markdown files in one floating editor, with no database or extra files.
    - Notes is a collection of plain Markdown files, as many as you want, edited in one floating window.
    - Turn it on in Settings → Notes. It's off by default, and turning it on doesn't create the folder or write anything until you make your first note.
    - _...and 4 more in this feature._
- [ ] **Files** (7)
    - By default, notes are stored in Tinycast's Application Support folder:
    - The documentation shows this example:\n\n```\n~/Library/Application Support/com.tinycast.app/Notes/\n```
    - To keep notes in another folder, click Choose… next to Notes Folder in Settings → Notes. Tinycast uses that folder as it is and doesn't move notes ...
    - _...and 4 more in this feature._
- [ ] **In The Menu Bar** (1)
    - Turn on Show in Menu Bar in Settings → Notes to add a Notes icon to the menu bar. It's off by default. Click the icon to show or hide your notes. R...
- [ ] **In The Window** (9)
    - The title bar has three buttons: Create, Browse and Open Folder.
    - ⌘N — Create a note.
    - ⌘P — Open the switcher, or return to it.
    - _...and 6 more in this feature._
- [ ] **Saving And One Important Caveat** (5)
    - Changes are saved automatically 300 ms after you stop typing.
    - Saving overwrites the file on disk. Tinycast doesn't watch the folder for changes, so if you edit the note that's open in Tinycast in another app, ...
    - ⌘O makes this easy to do by accident. It's the tradeoff for storing notes as plain files instead of in a database.
    - _...and 2 more in this feature._
- [ ] **The Editor** (22)
    - Markdown is formatted as you write. Headings, bold, italic, strikethrough, inline code, links, lists, tasks, quotes, code blocks and horizontal rul...
    - Images and syntax highlighting in code blocks aren't rendered; they appear as text. Tables also appear as text, in a fixed-width font without other...
    - **Click a checkbox** to check or uncheck a task. The text cursor stays where it was.
    - _...and 19 more in this feature._
- [ ] **The Formatting Bar** (4)
    - There's a round button at the bottom right of each note. Click it, or press ⌥⌘T, and a button for each of the shortcuts above appears, along with a...
    - To hide it, turn off Show Formatting Bar in Settings → Notes. The shortcuts still work. The bar is also hidden while Render Markdown is off.
    - Typing, selecting, copy and paste, Find, emoji, input methods, and undo and redo (⌘Z and ⇧⌘Z) all work like they do in any Mac text field. An empty...
    - _...and 1 more in this feature._
- [ ] **The Switcher** (2)
    - The switcher opens below the title bar. With nothing typed, it lists notes by when you last changed them. When you type, it searches titles and not...
    - Each row has VoiceOver actions to open, rename and move that note to the Trash.
- [ ] **The Window** (3)
    - The window's height fits the note. It grows and shrinks as you write until it reaches its maximum height or the edge of the screen, and then the no...
    - You can delete every note; the window then shows an empty state, and Create Note still works.
    - Hiding the window returns you to the app you were using, but only if that app is still in front. If you've already switched to something else, you ...

## Features > Snippets

_59 rows to build._

- [ ] **Commands** (13)
    - Reusable Markdown templates with placeholders, arguments and keyword expansion.
    - A snippet is text you reuse, saved as a plain Markdown file that you can also edit by hand.
    - Paste a snippet from the launcher or the snippet browser, or type its keyword in any app and it expands in place.
    - _...and 10 more in this feature._
- [ ] **How The Text Gets Inserted** (3)
    - Tinycast first tries a single replacement through Accessibility, then checks that it worked.
    - Some apps, like Chrome, VS Code, Slack and other Electron apps, don't support that properly, so Tinycast types the text instead. Short, single-line...
    - Longer or multi-line text is pasted using the clipboard for a moment. Tinycast saves your clipboard, pastes only the snippet text, then restores yo...
- [ ] **In The Launcher** (2)
    - Enabled snippets appear in launcher search while Show in launcher is on. Both the name and the keyword are searchable, and they rank the same way a...
    - Return pastes the snippet, and ⌘E edits it.
- [ ] **Keyword Expansion** (10)
    - Give a snippet a keyword, and typing that keyword in any app replaces it with the snippet.
    - Priority.
    - The typing buffer holds at most 256 characters. It resets when you switch apps, when Secure Event
    - _...and 7 more in this feature._
- [ ] **Limits** (3)
    - Each argument is asked for once, in the order it first appears, including arguments inside referenced snippets. Values are inserted as they are: te...
    - Snippet references aren't case-sensitive and can be nested up to five levels deep. Tinycast detects loops. A missing, looping or too-deeply nested ...
    - All {cursor} tokens are removed, and the first one sets where the cursor goes.
- [ ] **Modifiers** (1)
    - Json-stringify escapes text for use inside a JSON string, without adding the quotes. raw only matters in quicklinks. {cursor} and snippet reference...
- [ ] **Placeholders** (2)
    - These match Raycast's dynamic placeholders, so snippets you import keep working. Raycast's {selectedText} and {query} also work, as {selection} and...
    - The editor's Insert… menu lists every token. You type parameters and modifiers yourself.
- [ ] **Shortcuts** (3)
    - Any snippet can have its own global shortcut. Record it on the snippet's row in Settings → Snippets. Pressing it pastes the snippet wherever you're...
    - A snippet's shortcut does nothing while snippets are off or while that snippet is disabled.
    - The shortcut is tied to the file. If you rename or move the file outside Tinycast, or choose a different snippets folder, the shortcut is cleared. ...
- [ ] **The Editor** (9)
    - The editor opens in the launcher, from Create Snippet, Edit Snippet, or the New Snippet and edit buttons in Settings → Snippets. The template is on...
    - ⌘return — Save Snippet.
    - Return — Start a new line in the template.
    - _...and 6 more in this feature._
- [ ] **The File Format** (2)
    - Each snippet is one Markdown file in Tinycast's Application Support folder. To use another folder, click Choose… next to Snippets Folder in Setting...
    - Each channel has its own folder, so the stable and beta versions never share snippet files.
- [ ] **The Snippet Browser** (9)
    - Search Snippets lists every enabled snippet with a preview next to it. Type to filter by name or keyword.
    - The preview shows the template as written, with its placeholders, along with the name, keyword, shortcut, file name and character count. The previe...
    - Paste Snippet — return.
    - _...and 6 more in this feature._
- [ ] **When A Token Is Wrong** (2)
    - If Tinycast can't read a token, it leaves it in the text exactly as you wrote it instead of silently removing it. So if you see {arguemnt} in your ...
    - {browser-tab} and {calculator} aren't supported.

## Reference > Backup

_49 rows to build._

- [ ] **A Backup Can Never Turn On A Capability** (12)
    - Some switches are never included in a backup, because turning them on means agreeing to something, not just setting a preference:
    - **Snippets**: agreeing to keyword matching
    - **Extensions**: agreeing to run third-party code
    - _...and 9 more in this feature._
- [ ] **Imports Always Show What They Did** (5)
    - Every import shows a summary of what changed, by category.
    - If the file contains custom commands, Tinycast warns you before adding them, because a file that adds shell commands without telling you is a secur...
    - Imports add to what you have instead of overwriting it. Snippets and notes are added next to your existing ones, and a note with a name that's alre...
    - _...and 2 more in this feature._
- [ ] **Other Things That Aren T Included** (9)
    - Extensions: their code, data and sign-ins — They're third-party code, and their sign-ins are in the Keychain.
    - AI chats, API keys, and all AI and MCP settings — Conversations and keys stay on the Mac where they were created.
    - Quick Actions model, prompts, language and custom actions — An import should never change what a shortcut does to your text.
    - _...and 6 more in this feature._
- [ ] **Read This Before Relying On It** (11)
    - Save your setup to one file, choose what to export and what to import, and what a backup never does.
    - Settings → Backup.
    - A backup is a single .tinycast file. You choose what goes into it when you export, and separately choose what to restore when you import. So a back...
    - _...and 8 more in this feature._
- [ ] **Settings File** (5)
    - The Sync settings file switch in Settings → Backup → Settings File keeps a copy of your settings in ~/.config/tinycast/settings.json. It's off by d...
    - While it's on, a change you make in Settings is written to the file, and an edit you save in the file applies in Tinycast right away. If the file i...
    - The file holds your preferences, the shortcuts and aliases of apps, System Settings panes, system actions and built-in commands, and all of window ...
    - _...and 2 more in this feature._
- [ ] **Where Your Data Is Stored** (7)
    - Everything a backup includes is also a regular file. By default, it's in ~/Library/Application Support/com.tinycast.app/:
    - Snippets — Snippets/, or the Snippets Folder you chose.
    - Notes — Notes/, or the Notes Folder you chose.
    - _...and 4 more in this feature._

## Reference > Deeplinks

_45 rows to build._

- [ ] **Extension Links** (1)
    - Extension commands use their own links, like raycast://extensions///. See Compatibility.
- [ ] **Get A Link** (10)
    - Run any command, note or chat from a tinycast:// link, with text and arguments, from Karabiner, Shortcuts, scripts or a web page.
    - Anything you can give a shortcut, you can also run from a link. So can a note, a saved AI chat and a window size. Use links from Karabiner-Elements...
    - The documentation shows this example:\n\n```\nopen -g \"tinycast://command/clipboard-history\"\n```
    - _...and 7 more in this feature._
- [ ] **Karabiner Elements** (3)
    - Use open -g, so the link runs without bringing Tinycast forward:
    - The documentation shows this example:\n\n```\n{\n  \"description\": \"Hyper+V opens Tinycast clipboard history\",\n  \"manipulators\": [\n    {\n  ...
    - Window commands also work with a plain open. Snippets and Paste Sequentially need open -g, so the text goes to the app you're in.
- [ ] **Links That Ask First** (7)
    - A link from another app or a web page asks before it runs, if it carries text or arguments, or if it acts outside Tinycast. Acting outside Tinycast...
    - **Always Allow**: run it now, and never ask again for this command with this exact input.
    - **Run Once**: run it now, and ask next time.
    - _...and 4 more in this feature._
- [ ] **Send Text And Arguments** (15)
    - Links take three parameters. Encode their values for a URL.
    - **`fallbackText`** types text into the command's first field. For example,
    - Tinycast://command/search-files?fallbackText=~/Downloads/ opens Search Files already searching.
    - _...and 12 more in this feature._
- [ ] **What Links Can Do** (9)
    - Tinycast:// — Opens the launcher.
    - Tinycast://search?fallbackText=2%2B2 — Opens the launcher with 2+2 typed.
    - Tinycast://command/clipboard-history — Runs a built-in command.
    - _...and 6 more in this feature._

## Reference > Import From Raycast

_41 rows to build._

- [ ] **About The Passphrase** (3)
    - Raycast encrypts the export even if you never set a password. It creates one and stores it in your login Keychain.
    - You can find it in Raycast → Settings → Extensions → Export Settings & Data, or in Keychain Access under the service Raycast and account exportpass...
    - Tinycast never reads your Keychain. You paste the passphrase yourself.
- [ ] **Afterward** (2)
    - While Raycast is running, the pane shows a Quit Raycast button, so its shortcuts stop clashing with yours.
    - Tinycast doesn't need Raycast to be installed, except to import extensions from it, since that reads Raycast's folder.
- [ ] **Clipboard History** (2)
    - Pinned text and image entries stay pinned, so they survive the configured history retention even when their original copy dates are old. Imported p...
    - Images are only imported if their files still exist on this Mac. The summary tells you how many were missing.
- [ ] **Other Things You Can Import** (5)
    - Two things aren't part of a .rayconfig, so they have their own importers:
    - **Extensions.** **Settings → Extensions → Import from Raycast** copies your installed extensions.
    - See Installing extensions.
    - _...and 2 more in this feature._
- [ ] **Quicklinks** (2)
    - Quicklinks are added to your existing library, never replacing it, and duplicates are skipped the same way as in a quicklink import. Raycast's {Que...
    - Importing at least one quicklink turns on the Quicklinks feature.
- [ ] **Snippets** (4)
    - Snippets are added in their original order, without overwriting any you already have. Snippets that can't be read are skipped. If a name is already...
    - Imported snippets are enabled and visible in the launcher, with confirmation off.
    - Importing never turns on keyword expansion. Only you can turn that on; see Backup. The summary mentions this, so a keyword that doesn't work yet do...
    - _...and 1 more in this feature._
- [ ] **Steps** (6)
    - In Raycast, export your settings and data, and note the passphrase.
    - In Tinycast, go to **Settings → Backup → Raycast Export** and choose the file.
    - Enter the passphrase, select what you want to import, and import.
    - _...and 3 more in this feature._
- [ ] **What Gets Imported** (13)
    - Shortcuts — App shortcuts, command shortcuts, and your Hyper key setup.
    - Favorites — From Raycast's pinned items.
    - Aliases — App aliases.
    - _...and 10 more in this feature._
- [ ] **Which Exports It Reads** (4)
    - Import your shortcuts, favorites, snippets, quicklinks and clipboard history from a Raycast export.
    - Tinycast reads Raycast's export file directly. Import it in Settings → Backup → Raycast Export, or from the welcome screen the first time you open ...
    - Only Raycast v2.0 and newer. Tinycast reads the .rayconfig file written by Raycast v2.0 and later, and no other format. Check your Raycast version ...
    - _...and 1 more in this feature._

## Features > Navigation

_33 rows to build._

- [ ] **Good To Know** (7)
    - **It always acts on the app that was in front when you opened it.** Switching apps afterward
    - Doesn't change the target.
    - **Only items you could click are listed.** Disabled items, separators and hidden items are left
    - _...and 4 more in this feature._
- [ ] **Settings** (4)
    - Show Apple menu items — Off — Adds the Apple menu, which is the same in every app.
    - Excluded Applications — Empty — Apps whose menus Tinycast never reads.
    - Tinycast checks the Excluded Applications list before reading any menus, so it never reads those apps' menus at all.
    - _...and 1 more in this feature._
- [ ] **Switch Windows** (22)
    - Switch to any open window, or click any menu bar item, from the launcher.
    - Navigation adds three keyboard commands:
    - **Switch Windows** lists every open window of every running app and brings the one you choose to
    - _...and 19 more in this feature._

## The Overview

_30 rows to build._

- [ ] **1 Pick A Shortcut** (4)
    - Tinycast doesn't come with a shortcut, so it can't take over a key combination you already use. Until you choose one, open the launcher from the me...
    - The welcome screen asks for a shortcut. To change it later, go to Settings → General → Global Shortcuts → Show Launcher, click the field and press ...
    - You can also double-tap a modifier key, like pressing ⌘ twice. See Hotkeys.
    - _...and 1 more in this feature._
- [ ] **2 Allow Pasting Optional** (1)
    - Tinycast asks for Accessibility access so it can paste a clip or an emoji into the app you were using. The launcher itself doesn't need any permiss...
- [ ] **3 Bring Your Raycast Setup Optional** (3)
    - If you used Raycast, choose its .rayconfig export and enter the passphrase you set when you exported it. You choose what to bring: shortcuts, favor...
    - When you finish, Get Started opens the launcher.
    - Tinycast uses your Mac's language if it has a translation for it, and English otherwise. To change it, go to Settings → General → Language.
- [ ] **Learn One Shortcut First** (3)
    - Everything happens in one window. Type to search, press return to act on the selection, press ⌘K to see every other action, and press esc to go back.
    - ⌘K is the one to remember. Every screen lists all of its actions there, each with its shortcut, so it's the fastest way to learn the app.
    - Next, read about the palette, or install Tinycast if you haven't yet.
- [ ] **Set It Up** (4)
    - What Tinycast is and how to set it up.
    - Tinycast is a small, native launcher for macOS that lives in your menu bar. Press your shortcut and a palette opens over whatever you're doing. Typ...
    - It's built with SwiftUI and AppKit, has no third-party dependencies, and uses less than 100 MB of memory. It doesn't use Electron, and there's no a...
    - _...and 1 more in this feature._
- [ ] **Turn On What You Want** (15)
    - Most features are off when you install Tinycast. A feature that's off does nothing: it doesn't scan, index or use memory. That's how the app stays ...
    - AI Chat — Settings → AI.
    - Quick Actions — Settings → Quick Actions.
    - _...and 12 more in this feature._

## Launcher > System Actions

_28 rows to build._

- [ ] **A Few Details** (5)
    - Volume. Turn Volume Up and Turn Volume Down move in 5% steps: from 37%, up goes to 40% and down goes to 35%. Tinycast shows its own volume indicato...
    - Eject All Disks ejects external and removable drives, including a hard drive connected through a dock. It never ejects internal or network volumes.
    - Toggle Microphone Mute changes the default input device's own mute setting. If your microphone doesn't support software mute, or its mute control i...
    - _...and 2 more in this feature._
- [ ] **Confirmation** (3)
    - Return runs the action and esc cancels. Each dialog shows the action's icon, so you can see what you're about to do. Quit All Applications also tel...
    - Empty Trash follows your Finder setting. It only asks while Show warning before emptying the Trash is on in Finder ▸ Settings ▸ Advanced. If you tu...
    - The same confirmation appears when you use a shortcut. You can't skip it, and holding down a shortcut never opens more than one dialog.
- [ ] **Permissions** (1)
    - Some actions need Automation, Accessibility or Bluetooth access. Tinycast asks the first time you run an action that needs it, never in advance. If...
- [ ] **Settings** (2)
    - Settings → System Actions has Enable System Actions at the top and a row for each action, with a launcher checkbox, a shortcut recorder and an alia...
    - Turning off Enable System Actions removes every action from search and turns off their shortcuts. Clearing a single row's checkbox only hides that ...
- [ ] **The Full List** (15)
    - 32 built-in actions for your Mac, like lock, sleep, volume, microphone mute, Bluetooth and the Trash.
    - System actions control your Mac itself, like locking the screen or emptying the Trash. You can search for each one, and each one can have a global ...
    - They have their own System Actions section in the launcher and their own pane in Settings → System Actions.
    - _...and 12 more in this feature._
- [ ] **What You See Afterward** (2)
    - Actions without a visible effect show a short message with the result, like Trash Emptied, Hidden Files Shown, Dark Appearance, Bluetooth Off or 3 ...
    - A green check means something changed. A plain dot means there was nothing to do. For example, Trash Is Already Empty isn't an error, and neither a...

## Features > Emoji

_25 rows to build._

- [ ] **Actions** (10)
    - A searchable grid of every emoji and symbol, with skin tones, pins and your most-used characters first.
    - Search emoji and symbols, then paste one into the app you were using.
    - Open it with the Search Emoji & Symbols command, or with its own global shortcut from Settings → Emoji & Symbols.
    - _...and 7 more in this feature._
- [ ] **Frequently Used** (1)
    - The emoji you used most recently appear in a Frequently Used section below Pinned, up to two rows, so you don't have to search for them. They also ...
- [ ] **Grid Size** (1)
    - The Actions menu has Actual Size (⌘0), Zoom In (⌘+) and Zoom Out (⌘-), from six to ten columns. Set the default grid size in Settings → Emoji & Sym...
- [ ] **Moving Around The Grid** (4)
    - This is the only screen where all four arrow keys and all four Emacs chords move the selection. Here, ⌃F and ⌃B move through the grid instead of mo...
    - Moving into the first row scrolls all the way to the top, so the section title stays visible.
    - You can also use the mouse to click, double-click, right-click and hover.
    - _...and 1 more in this feature._
- [ ] **Pinned** (1)
    - Pin an emoji or symbol from the Actions menu to keep it at the top of the picker. New pins are added to the end of the section. You can reorder the...
- [ ] **Skin Tone** (8)
    - Settings → Emoji & Symbols → Emoji Skin Tone.
    - Default: Light.
    - Default: Medium Light.
    - _...and 5 more in this feature._

## Install

_21 rows to build._

- [ ] **Channels** (5)
    - Each channel is a separate app with its own settings, permissions and login item. They can run side by side, so you can keep the stable version and...
    - Stable — tinycast — Tinycast.app.
    - Beta — tinycast@beta — Tinycast Beta.app.
    - _...and 2 more in this feature._
- [ ] **Downloading Directly** (2)
    - Builds are also on the Releases page.
    - Open the DMG and drag Tinycast to Applications. Every release is signed with Tinycast's Apple Developer ID and notarized by Apple, so macOS opens i...
- [ ] **Homebrew** (4)
    - Homebrew, release channels, direct downloads, and updates.
    - Tinycast needs an Apple silicon Mac with macOS 26 or later.
    - Homebrew is the easiest way to install Tinycast.
    - _...and 1 more in this feature._
- [ ] **Uninstalling** (6)
    - The documentation shows this example:\n\n```\nbrew uninstall --cask tinycast\n```
    - To also remove the files Tinycast created, add --zap:
    - The documentation shows this example:\n\n```\nbrew uninstall --zap --cask tinycast\n```
    - _...and 3 more in this feature._
- [ ] **Updating** (4)
    - Tinycast updates itself. Once a day it checks for a new release on its own channel. When there is one, a window shows what changed. Update Now down...
    - To stop the daily check, turn off Automatically check for updates in Settings → General. Checking by hand still works.
    - Because the app manages its own updates, brew upgrade skips Tinycast. This is expected. See Updates for details.
    - _...and 1 more in this feature._

## Launcher > Favorites

_19 rows to build._

- [ ] **Adding One** (4)
    - Pin the things you open most, and open them by number.
    - Favorites appear above everything else when you open the launcher with nothing typed.
    - Select an entry and press ⇧⌘F, or choose ⌘K → Add to Favorites. The same shortcut and menu item remove it.
    - _...and 1 more in this feature._
- [ ] **Favorites And Ranking** (2)
    - Opening a favorite with nothing typed, or by its number, counts as a use in learned ranking. It doesn't teach Tinycast a search word, since you did...
    - Favorites are included in backups, and the Raycast importer can bring them over.
- [ ] **Files And Folders** (7)
    - Files and folders can be favorites too, while File Search is on. Select one in Search Files, or in the launcher's Files section, and press ⇧⌘F. It ...
    - A favorite file is found by its name at any length, even with File Search's **Show in launcher**
    - Off. It shows in the Files section, above other matches.
    - _...and 4 more in this feature._
- [ ] **In Compact Mode** (1)
    - The slim search bar of compact mode doesn't show your favorites, but the same numbers still open them. Press ↓ to open the full list.
- [ ] **Launching By Number** (3)
    - Hold ⌘ for a moment and each numbered favorite shows its number on the right side of its row.
    - The numbers only work while the Favorites section is visible, which means with nothing typed. Once you start typing, the list shows ranked results,...
    - In the clipboard screen, the same number keys paste your pinned entries instead.
- [ ] **Reordering** (2)
    - Favorites keep the order you give them. With nothing typed, select one and press ⌥⌘↑ or ⌥⌘↓, or choose ⌘K → Move Favorite Up or Move Favorite Down.
    - The selection moves with the favorite, so you can keep pressing the keys until it's where you want it.

## Launcher > Aliases

_18 rows to build._

- [ ] **Backups And Cleanup** (3)
    - Aliases are included in backups.
    - An alias is removed along with the thing it names. If you uninstall the app, delete the quicklink or remove the extension, its alias is deleted too...
    - Raycast exports include an alias for each command. The importer brings over the aliases for apps.
- [ ] **How An Alias Ranks** (8)
    - **Typing the whole alias exactly always puts that entry first**, no matter what you picked before
    - Or what else has that name.
    - **Typing the start of an alias** ranks just above names that start the same way. Something you
    - _...and 5 more in this feature._
- [ ] **In The List** (1)
    - An entry with an alias shows it as a small tag after its name, so you can see which entries you've named.
- [ ] **Setting One** (6)
    - Give any launcher entry a short name that comes first when you type it.
    - An alias is your own short name for a launcher entry. Type ps to get Photoshop, or sh to get the shell script you run every morning.
    - Aliases work on any entry: apps, System Settings panes, commands, Quick Actions, quicklinks, snippets, Apple Shortcuts, system actions, window comm...
    - _...and 3 more in this feature._

## Launcher > System Settings

_16 rows to build._

- [ ] **Actions** (8)
    - Search every macOS Settings pane from the launcher.
    - Tinycast lists System Settings panes as regular launcher entries. Type bluetooth or display and the pane opens directly, without opening System Set...
    - Panes have their own System Settings section, right after Applications. Pane names use your Mac's language, and the English names still match.
    - _...and 5 more in this feature._
- [ ] **Settings** (8)
    - Settings → System Settings lists every pane, with:
    - **Enable System Settings** at the top, which removes every pane from search and turns off their
    - Shortcuts.
    - _...and 5 more in this feature._

## Launcher > Fallbacks

_15 rows to build._

- [ ] **Open In Browser** (2)
    - Open in Browser works the other way around. When your text looks like a web address, like https://… or github.com/user/repo, Open in Browser appear...
    - It doesn't affect learned ranking and can't be a favorite, because it only exists for the text you typed.
- [ ] **Quicklinks As Fallbacks** (1)
    - Every enabled quicklink with an {argument} in its link becomes a fallback. Your text fills in the first argument. If that was the only value the li...
- [ ] **Run Shell Command** (2)
    - Your text runs in /bin/zsh from your home folder with your shell configuration loaded, so your own aliases like ll work. A window shows the output ...
    - This is separate from custom commands. Turning off custom commands doesn't hide it; only its own checkbox in Settings does.
- [ ] **Settings** (3)
    - Settings → Fallbacks lists every available fallback, each with a checkbox. Use the arrow buttons to change their order.
    - Fallbacks for features that are turned off don't appear here or in search. Hiding Define Word in Settings → Commands, or turning off Enable Command...
    - The order and checkboxes are never included in a backup, so importing a file can't add a shell command runner to your launcher.
- [ ] **The Built In Fallbacks** (7)
    - Send what you typed to Quick AI, file search, the dictionary, the shell or a quicklink.
    - Below every search, Tinycast shows a few rows under Use \"…\" with. Each one sends what you typed somewhere else. They always appear at the bottom,...
    - Quick AI — Sent as a question in the palette — AI is on.
    - _...and 4 more in this feature._

## Out of scope by decision

These rows stay in the ledger as NOT PORTED, with the reason recorded. Delete a
cut from parity/status.toml and they come back into scope.

- **Ai** (139) - Out of scope by decision (2026-11-11): YAL talks to no model provider and holds no API key, so the AI features are not built. The AI rows inside the commands and the shortcut reference went with them.
- **Launcher > Commands** (6) - Out of scope by decision (2026-11-11): YAL talks to no model provider and holds no API key, so the AI features are not built. The AI rows inside the commands and the shortcut reference went with them.
- **Reference > Shortcuts** (23) - Out of scope by decision (2026-11-11): YAL talks to no model provider and holds no API key, so the AI features are not built. The AI rows inside the commands and the shortcut reference went with them.
- **Ai > Mcp** (52) - Out of scope by decision (2026-11-11): YAL talks to no model provider and holds no API key, so the AI features are not built. The AI rows inside the commands and the shortcut reference went with them.
- **Permissions** (46) - Out of scope by decision (2026-11-11): the macOS accessibility and screen-recording permissions have no Windows answer. A per-user Windows install asks for nothing beyond what Windows already grants it.
- **Ai > Quick Actions** (45) - Out of scope by decision (2026-11-11): YAL talks to no model provider and holds no API key, so the AI features are not built. The AI rows inside the commands and the shortcut reference went with them.
- **Features > Dictation** (45) - Out of scope by decision (2026-11-11): dictation is not built. Windows' own speech input does the same job, and YAL does not build a second one.
- **Features > Window Layouts** (36) - Out of scope by decision (2026-11-11): the saved window layouts and the rooms that hold them are not built. The window management itself - switching, moving and resizing a window - stays.
- **Features > Navigation** (2) - Out of scope by decision (2026-11-11): macOS has a menu bar and YAL does not, so the menu items of the app in front cannot be read. Switching windows stays.
- **Launcher > Uninstall** (32) - Out of scope by decision (2026-11-11): the in-app uninstaller is not built. The installer still removes the program and keeps the data.
- **Features > Rooms** (28) - Out of scope by decision (2026-11-11): the saved window layouts and the rooms that hold them are not built. The window management itself - switching, moving and resizing a window - stays.
- **Extensions > Installing** (27) - Out of scope by decision (2026-11-11): Raycast extensions and Apple Shortcuts are macOS concepts. YAL carries neither.
- **Extensions** (26) - Out of scope by decision (2026-11-11): Raycast extensions and Apple Shortcuts are macOS concepts. YAL carries neither.
- **Reference > Updates** (25) - Out of scope by decision (2026-11-11): YAL is built and used by one person, who rebuilds it from the source, so there is no update to check for.
- **Features > Apple Shortcuts** (22) - Out of scope by decision (2026-11-11): Raycast extensions and Apple Shortcuts are macOS concepts. YAL carries neither.
- **Extensions > Compatibility** (21) - Out of scope by decision (2026-11-11): Raycast extensions and Apple Shortcuts are macOS concepts. YAL carries neither.
- **Extensions > Customising** (21) - Out of scope by decision (2026-11-11): Raycast extensions and Apple Shortcuts are macOS concepts. YAL carries neither.
- **Features > Camera** (14) - Out of scope by decision (2026-11-11): the camera is not built.
- **Features > Dictionary** (13) - Out of scope by decision (2026-11-11): the dictionary is not built. The Define Word command opens a web lookup instead.
