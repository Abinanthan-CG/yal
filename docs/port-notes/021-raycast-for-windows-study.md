# 021 - What Raycast for Windows is made of, and what its calculator does

Date: 2026-11-13
Subject: Raycast for Windows 2.12.0 (`Raycast.Raycast_2.12.0.0_x64__qypenmj9wpt2a`)
Tools: `borer` 0.2.12 (native reverse engineering over MCP) and `ilspycmd` 9.1.0.7988
(managed decompilation)

## Why this note exists

The port's authority is upstream Tinycast, whose source we have. Raycast is a
different product, and a much larger one: it is the app whose calculator comfort
made this port worth building. Reading it tells us two things: where our own
calculator is thin, and how a Windows launcher of this kind is put together.

**Nothing here is copied into YAL.** What was taken is *behaviour* - which
features exist, which words the parser knows, how many units there are and what
they convert to. Facts about units and exchange rates are facts; Raycast's code,
tables and naming stay in Raycast's binary. Every feature YAL takes on this
basis is written from scratch, in Rust, with its own tests.

## The tools, and what each one is for

- **ILSpy** reads .NET assemblies. Raycast for Windows is a .NET app, so the
  whole feature layer comes back as readable C# - no reverse engineering needed.
- **borer** reads native code: PE, ELF and Mach-O, to pseudo-C. Raycast's
  calculator is a Rust library, so this is the tool for that half.

Rule of thumb this study settled on: **read the managed code with ILSpy, and
reach for borer only where the logic is in a native library.** Starting with
borer on a .NET assembly would have been the slow way round.

## The anatomy of Raycast for Windows 2.12.0

```
Raycast.exe                 the .NET host (WPF)
Raycast.dll        9.2 MB   the feature layer: 4038 types
native.dll         7.5 MB   a Rust core, exposed to .NET over UniFFI
backend/node.exe            a Node runtime for the extension host
backend/calculator-rs.*.node 4.7 MB  the calculator, a Rust N-API addon
backend/indexer-*.node      the file index
backend/resources/ripgrep   search
```

The .NET layer holds `Raycast.IPC` (693 types), `Raycast.Handlers` (627),
`Raycast.Platform` (421) and `Raycast.Features` (403). The native core is
reached through `uniffi.*` bindings, in the namespaces `native`,
`raycast_host`, `display`, `window_management`, `system_cursors` and
`shortcuts`.

The calculator is *not* in the .NET layer: it is `calculator-rs`, a Rust N-API
addon with one export (`napi_register_module_v1`), 6513 functions, built with
`rustc` 1.9x and MSVC 14.44, signed "Raycast Technologies Ltd.".

## What the calculator does

This is the useful part, and it is much more than upstream Tinycast's.

**Arithmetic.** Addition, subtraction, multiplication, division, powers (`^`),
percentages, brackets, comparisons (`==`, `!=`, `<`, `>`, `<=`, `>=`) and bitwise
operators (`&`, `|`, `<<`, `>>`, `xor`, `~`). Four number bases: decimal, hex,
binary and octal.

**Units.** 617 units in 53 families, each with its own aliases and its
conversion factor to the family's base unit:

| | |
| --- | --- |
| length, mass, duration, area, volume | the everyday ones |
| speed, acceleration, force, pressure, energy, power, torque | the physical ones |
| data storage, data transfer, frame rate, pixel density | the computer ones |
| electric charge, current, potential, resistance, conductance, inductance, capacitance | the electrical ones |
| magnetic flux, flux density, field strength | the magnetic ones |
| radioactivity, absorbed dose, dose equivalent, exposure | the radiological ones |
| illuminance, luminance, luminous intensity, solid angle | the light ones |
| dynamic and kinematic viscosity, dispersion, concentration | the chemical ones |
| fuel economy, fuel consumption, pace | the everyday compound ones |

Each unit knows its aliases in English and in the other spellings the app
supports (`meter`/`metre`, `liter`/`litre`, `celcius`/`celsius`).

**Money.** A currency table with the fiat codes, the crypto tickers, and the
currency's name in several languages - `United State Dollar`, `liberian dollar`,
`libra de gibraltar`, `kip laotien`. Rates are fetched, and the calculator can
ask for a **historical** rate: `historicalCurrencyRateFetcher` and a
"historical currency provider" are in the binary.

**The world.** The calculator answers questions about the world, not just about
numbers: `weatherFetcher`, `travelTimeFetcher`, `flightFetcher` and
`placeFetcher` are all part of it, with a "future weather in arithmetic" and
"weather conditions in arithmetic" rule, and a full payload for a forecast
(temperature, apparent temperature, humidity, wind, sunrise, sunset, rain,
snow, moonrise, precipitation chance, cloud cover, visibility, dew point, UV
index, pressure) and for a flight (origin, destination, designator, date,
status).

**Time.** The whole time zone database, ISO weeks, week numbers, civil days,
business days and business hours (`workHoursPerDay`), calendar units, and the
holidays: Christmas, Christmas Eve, Boxing Day, New Year's Day and Eve,
Valentine's Day, Halloween, Easter (Western and Orthodox), Good Friday, Holy
Saturday, Thanksgiving, Black Friday, Ramadan, Chinese New Year, Hanukkah.

**Locale.** The input follows the locale: `inputGroupingSeparators` (so `1,000`
means a thousand in English and a decimal comma in German), the names of the
months and the weekdays, the era, the day periods, and `remToPxRate` - the
calculator converts CSS `rem` to pixels.

**The vocabulary.** The parser's words are visible in the binary's data
section, as one long string per group:

```
in as between on at now local distance plus minus per is for with until since times
and divided over mod
sum total count gcd gcf lcm greater lesser
monthly timespan today midday next last gmt utc timezone date stamp
subtractionOperator binaryNumber octalNumber hexadecimalNumber additionOperator
powerOperator percentageSymbol openBracket bracket closeBracket bitwiseAndOperator
bitwiseOrOperator bitwiseLeftShiftOperator bitwiseRightShiftOperator
lessThan greaterThan lessThanOrEqualTo greaterThanOrEqualTo equalTo notEqualTo
```

Two details worth stealing outright:

- **`per` means divide**, so `60 miles per hour` is `60 mi/hr` - and a rate is
  a unit of its own that survives being multiplied by a time.
- **`gcd`, `gcf` and `lcm`** are three spellings of the same two functions.

## What this changes for YAL

Our calculator is behind in four ways, and the ledger says so: the arithmetic
that carries a unit, the dates and time zones, the history, and the live rates.
The study says where to aim:

1. **The unit catalogue** is data, and it is the biggest single win: our table
   holds upstream Tinycast's units, and Raycast's holds 617 across 53 families.
   The conversion factors are facts; the catalogue is ours to write.
2. **The unit has to travel through the arithmetic.** Upstream's rule is "the
   last unit you type sets the unit of the answer", so `10kg + 500g` is
   `10,500 g` and `10kg + 500g to lb` is pounds. Our `Value` carries money but
   not units, so this is the change to make.
3. **A rate is a unit.** `$50/hr for 3 hours` is `$150`, and `60 km/h for
   30 min` is `30 km`. The dimension system our `units.rs` already has
   (length, mass and time exponents) is the right shape for it.
4. **The dates and time zones are a separate slice**, not a part of this one.
   Raycast keeps them in `jiff`; upstream Tinycast keeps them in `CalcDateTime`
   and `CalcTimeZone`. They are the next biggest block of open rows.

## What borer found in our own binary

The tool earns its keep here too. `borer info target/release/yal.exe` reads our
binary's resources, and the manifest it found did **not** carry a
`requestedExecutionLevel`, a `dpiAwareness` or a `longPathAware` entry - while
`docs/WINDOWS-MAPPING.md` claimed that "Windows long paths are enabled through
the manifest and the `longPathAware` setting".

The claim was true and the binary was not. Fixed in this session:
`apps/yal/windows/yal.manifest` now says `asInvoker`, `PerMonitorV2`,
`longPathAware` and UTF-8, and `apps/yal/build.rs` hands it to `tauri-build`.

The rebuilt binary carries it. `borer info` reads `requestedExecutionLevel=asInvoker`
back out of the resource, but borer's manifest reader does not parse the
`<application><windowsSettings>` block at all - it reports `dpiAware=`,
`dpiAwareness=` and `longPathAware=` as empty whatever the manifest says. So the
rest of the manifest was confirmed by pulling the `RT_MANIFEST` resource out of
the file itself: the whole document is there, `PerMonitorV2,PerMonitor`,
`longPathAware=true` and `activeCodePage=UTF-8` included.

That is the whole argument for having the tool: a promise in a document is not
a feature in a binary. It is also the argument for reading the tool's output
carefully: borer not printing a setting is not evidence the setting is absent.

## Verified

- `borer info` on `yal.exe` and on `calculator-rs.node`: format, architecture,
  sections, imports, exports, mitigations and the manifest resource.
- The launcher's manifest: `RT_MANIFEST` read out of the rebuilt
  `target/release/yal.exe`, whole document present, `asInvoker`,
  `PerMonitorV2,PerMonitor`, `longPathAware=true` and `activeCodePage=UTF-8`
  included.
- `ilspycmd -l` on `Raycast.dll`: 4038 types, none of them the calculator.
- The unit catalogue: 617 units in 53 families, rebuilt from the data section
  of `calculator-rs.node` and checked against the base unit of every family.
- The vocabulary: read out of the binary's data section, not guessed.
