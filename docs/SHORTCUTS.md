# Keyboard shortcuts

> Every built-in shortcut in the palette and its screens, translated to
> Windows, in one place.

These shortcuts are built in and cannot be changed. For shortcuts you record
yourself, see [Hotkeys](reference/HOTKEYS.md) and the *Hotkeys* section of
YAL's own settings.

---

## The translation

Tinycast is a macOS application. Windows has different modifier keys, so every
chord is translated:

| Tinycast | YAL |
|---|---|
| <kbd>⌘</kbd> Command | <kbd>Ctrl</kbd> |
| <kbd>⌥</kbd> Option | <kbd>Alt</kbd> |
| <kbd>⇧</kbd> Shift | <kbd>Shift</kbd> |
| <kbd>⌃</kbd> Control | <kbd>Alt</kbd> |
| <kbd>⌘</kbd> + <kbd>⌃</kbd> | <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Shift</kbd> |

### Why <kbd>⌃</kbd> becomes <kbd>Alt</kbd>

Windows already uses <kbd>Ctrl</kbd> for what macOS uses <kbd>⌘</kbd> for, and
<kbd>Alt</kbd> for what macOS uses <kbd>⌥</kbd> for. <kbd>⌃</kbd> therefore
becomes <kbd>Alt</kbd>.

That would make <kbd>⌥</kbd><kbd>⌘</kbd><kbd>C</kbd> and
<kbd>⌃</kbd><kbd>⌘</kbd><kbd>C</kbd> the same chord, so when
<kbd>⌃</kbd> appears together with <kbd>⌘</kbd> the whole chord becomes
<kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Shift</kbd>. That keeps every shortcut
in upstream distinct.

### The chords that are not a plain swap

| Tinycast | YAL | Why |
|---|---|---|
| <kbd>⌘</kbd><kbd>delete</kbd> | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Backspace</kbd> | <kbd>Ctrl</kbd>+<kbd>Delete</kbd> is "delete the word to the right" in a text field, and must keep working |
| <kbd>⌥</kbd><kbd>tab</kbd> | <kbd>Ctrl</kbd>+<kbd>Tab</kbd> | <kbd>Alt</kbd>+<kbd>Tab</kbd> is the Windows system shortcut for switching windows, and is never intercepted |
| <kbd>⌘</kbd><kbd>esc</kbd> | <kbd>Ctrl</kbd>+<kbd>Esc</kbd>, and <kbd>Alt</kbd>+<kbd>Home</kbd> | <kbd>Ctrl</kbd>+<kbd>Esc</kbd> opens the Windows Start menu; YAL only takes it while the palette has focus |

### The rule that outranks all of these

**A built-in shortcut must never shadow standard text editing.**
<kbd>Ctrl</kbd>+<kbd>C</kbd> is copy. <kbd>Ctrl</kbd>+<kbd>V</kbd> is paste.
<kbd>Ctrl</kbd>+<kbd>X</kbd> is cut. <kbd>Ctrl</kbd>+<kbd>Z</kbd> is undo.
Those work in every text field of the palette, and YAL never takes them over.

---

## Everywhere in the palette

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Main action |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Second action |
| <kbd>Ctrl</kbd>+<kbd>K</kbd> | Actions menu |
| <kbd>↑</kbd> <kbd>↓</kbd> | Move the selection |
| <kbd>Alt</kbd>+<kbd>N</kbd> / <kbd>Alt</kbd>+<kbd>P</kbd> | Same as <kbd>↓</kbd> / <kbd>↑</kbd> |
| <kbd>Alt</kbd>+<kbd>F</kbd> / <kbd>Alt</kbd>+<kbd>B</kbd> | Same as <kbd>→</kbd> / <kbd>←</kbd> |
| <kbd>Tab</kbd> | Launcher → Quick AI → clipboard, or move through fields |
| <kbd>Esc</kbd> | Clear the search, go back a screen, then close |
| <kbd>Ctrl</kbd>+<kbd>Esc</kbd> | Back to the root search from any screen |
| <kbd>Delete</kbd> | In an empty search, go back a screen |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | Settings |
| <kbd>Ctrl</kbd>+<kbd>W</kbd> | Close the palette |

---

## Launcher

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Open |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Show in File Explorer (app, setting, snippet) |
| <kbd>Shift</kbd>+<kbd>Ctrl</kbd>+<kbd>F</kbd> | Add to / Remove from Favorites |
| <kbd>Alt</kbd>+<kbd>Ctrl</kbd>+<kbd>↑</kbd> / <kbd>↓</kbd> | Move a favorite up or down |
| <kbd>Ctrl</kbd>+<kbd>1</kbd> … <kbd>Ctrl</kbd>+<kbd>0</kbd> | Open favorite 1 to 10 |
| <kbd>Shift</kbd>+<kbd>Ctrl</kbd>+<kbd>H</kbd> | Hide from Search |
| <kbd>Ctrl</kbd>+<kbd>R</kbd> | Restart Application (running apps only) |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>Q</kbd> | Quit Application (running apps only) |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Edit Quicklink, Snippet or Custom Command |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Put Answer in Search Bar (calculator) |
| <kbd>Shift</kbd>+<kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Copy Calculation, on a calculator card |

---

## Quick AI

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Send, or stop a reply that is streaming |
| <kbd>Ctrl</kbd>+<kbd>V</kbd> | Attach an image, PDF or text file |
| <kbd>Delete</kbd> | In an empty message box, remove the last attachment |

---

## Chat History

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Open Chat |
| <kbd>Alt</kbd>+<kbd>X</kbd> | Delete Chat |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd> | Delete All Chats |

---

## Quick Action panel

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Replace your selection |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | Copy the result |
| <kbd>Esc</kbd> | Close the panel |

---

## Clipboard

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Paste |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Copy to Clipboard |
| <kbd>Alt</kbd>+<kbd>Enter</kbd> | Paste and Keep Window Open |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | Filter by type |
| <kbd>Ctrl</kbd>+<kbd>.</kbd> | Pin / Unpin Entry |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Edit Entry |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | Show in File Explorer |
| <kbd>Ctrl</kbd>+<kbd>1</kbd> … <kbd>Ctrl</kbd>+<kbd>0</kbd> | Paste pinned entry 1 to 10 |
| <kbd>Alt</kbd>+<kbd>X</kbd> | Delete Entry |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd> | Delete All Entries |

When **Default action** is set to Copy to Clipboard, <kbd>Enter</kbd> and
<kbd>Ctrl</kbd>+<kbd>Enter</kbd> are swapped.

---

## Emoji

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Paste |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Copy to Clipboard |
| <kbd>Alt</kbd>+<kbd>Enter</kbd> | Paste and Keep Window Open |
| Arrow keys | Move through the grid, in all four directions |

---

## Calculator History

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Copy Answer |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Copy Expression, on a past entry |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Put Answer in Search Bar, on a new calculation |
| <kbd>Alt</kbd>+<kbd>X</kbd> | Delete Entry |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd> | Delete All Entries |

---

## File search

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Open File / Open Folder |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Show in File Explorer |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | Filter by type |
| <kbd>Alt</kbd>+<kbd>Ctrl</kbd>+<kbd>C</kbd> | Copy Name |
| <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>C</kbd> | Copy Path |
| <kbd>Alt</kbd>+<kbd>X</kbd> | Move to the Recycle Bin |

The actions menu (<kbd>Ctrl</kbd>+<kbd>K</kbd>) lists exactly these. Copying
the file itself, pasting it into another application and adding it to the
favorites are not offered, because they are not built yet: a screen that
offers an action that does nothing is a dead end.

---

## Switch Windows

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Switch to Window |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>M</kbd> | Minimize Window |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>H</kbd> | Hide Application |
| <kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>Q</kbd> | Quit Application |

Repeated presses of your recorded Switch Windows shortcut move through the
list; Switch Windows Backwards moves in reverse. After repeating with the
modifiers held, release them to switch.

---

## Quicklinks

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Open Quicklink |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Open With Default App |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Edit Quicklink |
| <kbd>Ctrl</kbd>+<kbd>D</kbd> | Duplicate Quicklink |
| <kbd>Ctrl</kbd>+<kbd>.</kbd> | Pin / Unpin |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | Show in File Explorer |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Backspace</kbd> | Delete Quicklink |

---

## Snippets

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Paste Snippet |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Show in File Explorer |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Edit Snippet |
| <kbd>Ctrl</kbd>+<kbd>N</kbd> | Create Snippet |

---

## Meeting card

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Join Meeting |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Copy Meeting Link |

---

## Uninstall

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Uninstall Application |
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Select / Unselect File |
| <kbd>Alt</kbd>+<kbd>Ctrl</kbd>+<kbd>C</kbd> | Copy Path |
| <kbd>Shift</kbd>+<kbd>Ctrl</kbd>+<kbd>O</kbd> | Show in File Explorer |
| <kbd>Shift</kbd>+<kbd>Ctrl</kbd>+<kbd>I</kbd> | Show the file's properties |

---

## Notes window

| Key | Does |
|---|---|
| <kbd>Ctrl</kbd>+<kbd>N</kbd> | Create a note |
| <kbd>Ctrl</kbd>+<kbd>P</kbd> | Open the switcher |
| <kbd>Ctrl</kbd>+<kbd>O</kbd> | Open the Notes folder |
| <kbd>Ctrl</kbd>+<kbd>W</kbd> | Hide the window |
| <kbd>Esc</kbd> | Close the switcher, then hide |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Backspace</kbd> | Move the selected note to the Recycle Bin |

---

## Switch Room

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Enter the room |
| <kbd>Ctrl</kbd>+<kbd>Tab</kbd> / <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Tab</kbd> | Next or previous layout that fits |
| <kbd>Ctrl</kbd>+<kbd>K</kbd> | Room actions |
| <kbd>Ctrl</kbd>+<kbd>N</kbd> | Create a room |
| <kbd>Alt</kbd>+<kbd>X</kbd> | Delete the room; its windows stay open |

In the window picker, <kbd>Enter</kbd> adds or removes a window and
<kbd>Ctrl</kbd>+<kbd>Enter</kbd> saves the room.

---

## Custom commands

| Key | Does |
|---|---|
| <kbd>Enter</kbd> | Run Command |
| <kbd>Ctrl</kbd>+<kbd>N</kbd> | Create Custom Command |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Edit Custom Command |
| <kbd>Alt</kbd>+<kbd>X</kbd> | Delete Custom Command |

For a command with arguments, <kbd>Tab</kbd> moves through its inline fields
and <kbd>Enter</kbd> focuses the first missing required value before running.

---

## Window layout editor

| Key | Does |
|---|---|
| <kbd>Ctrl</kbd>+<kbd>Enter</kbd> | Save |

---

## Dialogs

On **every** dialog, including ones that delete things, <kbd>Enter</kbd>
clicks the main button and <kbd>Esc</kbd> cancels. Cancel is always on the
left.

---

## Shortcuts you record yourself

YAL ships with no global shortcuts. You record every one yourself, in
*Settings → General → Show Launcher* and on each item's row.

- A shortcut needs <kbd>Ctrl</kbd>, <kbd>Alt</kbd>, <kbd>Shift</kbd> or the
  Windows key. Only the function keys, <kbd>F1</kbd> to <kbd>F20</kbd>, work
  without one.
- Press <kbd>Esc</kbd> to stop recording, or <kbd>Delete</kbd> to clear the
  shortcut.
- Recording a shortcut needs no permissions. Only *using* some kinds of
  shortcut does.

### Modifier keys on their own

Instead of a combination, a shortcut can be a single tap or a double tap of
one <kbd>Alt</kbd>, <kbd>Shift</kbd>, <kbd>Ctrl</kbd> or <kbd>Win</kbd> key.

- A **tap** is a press of exactly one modifier key, with no other key held
  and no click, released within **250 ms**.
- A **double tap** is a second tap of the same key that starts within
  **300 ms** of releasing the first.
- **The shortcut runs when you release the key, not when you press it.**
- If one key has both a single-tap and a double-tap shortcut, the single tap
  waits until the time for a second tap is over.

YAL only listens for modifier taps while at least one shortcut uses one, so if
you do not use them, they cost nothing.

### The Windows key

macOS has a Globe key that Windows does not have. The nearest equivalent is
the **Windows key**, and it is used as an ordinary modifier: <kbd>Win</kbd>
+ <kbd>Space</kbd> opens the palette by default on Windows.

The Windows key is deliberately **not** reserved. Windows itself owns a set of
chords with it — <kbd>Win</kbd>+<kbd>E</kbd> opens the Explorer, <kbd>Win</kbd>
+<kbd>D</kbd> shows the desktop, <kbd>Win</kbd>+<kbd>L</kbd> locks the
machine — and YAL does not take them over.

### The Caps Lock key

macOS's Hyper key, which turns one physical key into <kbd>⌃</kbd><kbd>⌥</kbd>
<kbd>⌘</kbd>, has no direct Windows equivalent, because remapping a key at the
hardware level is not something a normal Windows application is allowed to do.

YAL therefore has **no Hyper key**. The closest honest equivalent is to
record the modifiers you need on the shortcut itself.

What YAL does keep is the idea of the keycap: a shortcut that uses every
modifier YAL supports is shown as a single <kbd>✦</kbd>, and a shortcut that
uses a modifier on one side only shows a small **L** or **R**.

### Secure Input

macOS's Secure Input, where password fields hide every key press from
launchers, has a Windows equivalent: an administrator window or the secure
desktop. While one is active, YAL cannot see key presses, so its hotkeys do
not fire. This is the same behaviour, and it is not a bug.

---

## See also

- [Windows mapping](WINDOWS-MAPPING.md) — how each macOS concept is read on
  Windows
- [Architecture](ARCHITECTURE.md) — how the code is arranged
