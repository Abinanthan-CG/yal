# Port note 005 — The search slice, and the oracle it was built against

**Date.** 2026-11-11

---

## The problem

The launcher's search is the product. One query has to find the application,
the file, the setting, the snippet or the window the user meant, and it has to
order them the way the user expects.

Upstream keeps that in seven small, pure Swift types under
`Tinycast/Features/Launcher/Model/`. They are worth reading in this order,
because each one only makes sense against the one before it:

| Upstream | What it decides |
| --- | --- |
| `SearchRelevance.swift` (`FuzzyMatch` half) | the fold, and the tier a query hits a name at |
| `SearchRelevance.swift` (`SearchRelevance` half) | which alias matched, and how well it fits |
| `LauncherMatch.swift` | the score inside one alignment |
| `ScriptRomanization.swift` | how a name in another script is read |
| `EntryNaming.swift` | which names an entry is found by at all |
| `LauncherRankingStore.swift` | what the user opens, and by which query |
| `LauncherOrder.swift` | which entry comes first |

---

## The oracle

Upstream ships its own tests, and they are unusually good: they pin exact
numbers rather than describing behavior.

`upstream/Tests/fuzz-test.swift` pins:

- 19 exact matcher scores, such as `("chrome", "Google Chrome") == 13` and
  `("chrome", "Chrome Remote Desktop") == 14`
- the sensitivity levels, where High accepts a mid-word hit and Medium does not
- the transliterations, including `微信` → `weixin`, `wx`
- **25 comparator rules**, each one a sentence and a pair of entries
- **12 category rules**, such as `'window manager' expands no categories`

`upstream/Tests/ranking-test.swift` pins the frecency curve: 101 after one
visit, 50.5 after ten days, 150.5 after another visit, floored at 1; and the
learned terms, including the one that is easy to get wrong — a fresh open
brings the terms back after eighteen days of silence.

Every one of those is a test here, with upstream's own numbers. If a number
ever changes, the test says so.

---

## Three things that are easy to get wrong

### 1. The matcher works in UTF-16 units, not bytes

Upstream's `SearchText` holds `[UInt16]`, because macOS strings are UTF-16.
The humps — the offsets that mark a word start in `VisualStudioCode` — are
**byte** offsets into the UTF-8 form, and they are only computed when the name
is plain ASCII.

Rust strings are UTF-8, so the port keeps the units as `Vec<u16>` and converts
with `String::from_utf16_lossy`. The humps stay byte offsets, and a name in
another script gets no humps at all.

### 2. The alignment walks backwards from the query's last letter

`LauncherMatch.align` starts at the **end** of the query and walks back to the
start, keeping the best row in between. It never considers the rows before the
first matched letter. The bound is `width - (letters - 1 - matched)`, which
goes negative when the query is longer than the name — Swift's signed `Int`
allows that, and the port has to allow it too, or the subtraction underflows.

### 3. `ScriptRomanization.latin` returns nothing for a Latin name

This one cost the most time. Upstream's `latin` returns `nil` when the
transliteration equals the folded name — which is the case for every name
that is already Latin, including `Café Noir` and `ｃａｆｅ`. The caller,
`SearchText.string`, then falls back to the fold:

```swift
transliterated ? ScriptRomanization.latin(raw) ?? FuzzyMatch.normalized(raw)
                               : FuzzyMatch.normalized(raw)
```

So `SearchText("Café Noir", true).string == "cafe noir"`, but
`ScriptRomanization.latin("Café Noir") == nil`. Porting the rule rather than
the expected value is what makes the whole thing line up.

---

## The tables Windows has to carry

Upstream hands every non-Latin script to ICU's `toLatin`. Windows has no ICU,
so the port writes the tables itself:

| Script | How it is read |
| --- | --- |
| Han | the `pinyin` crate, toneless — `微信` → `wei xin` |
| Japanese | the kana table, with the katakana read as their hiragana twin |
| Hangul | the Revised Romanization tables |
| Cyrillic | upstream's own table, lowercased first |
| Greek | a table, because upstream hands Greek to ICU as well |

Two details that the tables have to get right:

- **A small tsu doubles the next consonant.** `きって` is `kitte`, not `kii`.
  The romanizations are not phonemic — `shi`, `chi` and `tsu` are one
  consonant each — so the onset is looked up rather than guessed from the
  first letter.
- **The Hangul initial `ㅇ` is silent.** `안녕하세요` is `annyeonghaseyo`,
  which is what the Revised Romanization says and what upstream's ICU
  transform produces.

Every other script is left alone: a name in Thai or Hebrew is still an exact
hit in its own script, which is the behavior that matters.

---

## The ledger learned a second file

The ledger's **scope** — which behaviors upstream documents — is generated,
because a hand-written list drifts from the docs. The port's **verdict** on
each row cannot be generated: it changes when the port ships.

So there are two files:

```text
parity/parts/generated/*.toml   the scope, from upstream's documentation
parity/status.toml               the verdict, written by hand
```

`cargo xtask parity` applies the second on top of the first, so regenerating
the scope never loses the verdict. A status entry naming a row that no longer
exists is reported as a problem rather than silently dropped.

The rule for a row is deliberately strict: **`PORTED` needs a test that ran
and shows the behavior the row describes**, not a piece of code that could
produce it. A matcher that can score an alias is not the same as a launcher
that finds an entry by that alias, and the difference is exactly what a reader
of `PARITY.md` is asking about.
