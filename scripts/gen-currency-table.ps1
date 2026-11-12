# Regenerates the Rust currency table from upstream's generated Swift data.
#
# Upstream's `CurrencyData.generated.swift` is produced by `Scripts/gen-currencies.js`
# from the CLDR data; this script transcribes that file into the Rust table the port
# uses, so the two cannot drift apart by hand.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gen-currency-table.ps1 `
#       -Upstream upstream/Tinycast/Features/Calculator/Model/CurrencyData.generated.swift `
#       -Target crates/tc-calc/src/currency/table.rs

param(
    [string]$Upstream = "upstream/Tinycast/Features/Calculator/Model/CurrencyData.generated.swift",
    [string]$Target = "crates/tc-calc/src/currency/table.rs"
)

$ErrorActionPreference = 'Stop'

$text = [System.IO.File]::ReadAllText($Upstream, [System.Text.Encoding]::UTF8)
$lines = $text -split "`r?`n"

function Get-Block([string]$marker) {
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $marker) { $start = $i; break }
    }
    if ($start -lt 0) { throw "block not found: $marker" }
    $end = $lines.Count
    for ($i = $start + 1; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^\s*\]') { $end = $i; break }
    }
    return $lines[($start + 1)..($end - 1)]
}

$all = Get-Block 'static let all'
$signs = Get-Block 'static let signs'
$aliases = Get-Block 'static let aliases'

$buffer = [System.Collections.Generic.List[string]]::new()
$buffer.Add("//! The currency table, transcribed from upstream's generated data.")
$buffer.Add("//!")
$buffer.Add("//! Mirrors upstream `CurrencyData.generated.swift`, which upstream's")
$buffer.Add("//! `Scripts/gen-currencies.js` writes from the CLDR data. Regenerate with")
$buffer.Add("//! `scripts/gen-currency-table.ps1` rather than by hand.")
$buffer.Add("")
$buffer.Add("/// Every fiat currency the rate feed prices, as (ISO 4217 code, display name).")
$buffer.Add("pub static FIAT: &[(&str, &str)] = &[")
foreach ($line in $all) {
    if ($line -match '^\s*\("([A-Z]{3})",\s*"(.*)"\),?\s*$') {
        $buffer.Add("    (`"$($matches[1])`", `"$($matches[2])`"),")
    }
}
$buffer.Add("];")
$buffer.Add("")
$buffer.Add("/// Currency sign to ISO code, lowercased to match the tokenizer's ident form.")
$buffer.Add("///")
$buffer.Add("/// Only signs CLDR assigns to exactly one currency, so the dollar sign is USD")
$buffer.Add("/// and the yen sign is JPY.")
$buffer.Add("pub static SIGNS: &[(char, &str)] = &[")
foreach ($line in $signs) {
    if ($line -match '^\s*"(.*)":\s*"([a-z]{3})",\s*$') {
        $buffer.Add("    ('$($matches[1])', `"$($matches[2])`"),")
    }
}
$buffer.Add("];")
$buffer.Add("")
$buffer.Add("/// Currency noun to ISO code, for nouns exactly one currency uses.")
$buffer.Add("///")
$buffer.Add("/// The contested ones - dollars, pounds, francs - are absent by design; the")
$buffer.Add("/// hand-written tables in the module above assign those.")
$buffer.Add("pub static ALIASES: &[(&str, &str)] = &[")
foreach ($line in $aliases) {
    if ($line -match '^\s*"(.*)":\s*"([A-Z]{3})",\s*$') {
        $buffer.Add("    (`"$($matches[1])`", `"$($matches[2])`"),")
    }
}
$buffer.Add("];")
$buffer.Add("")

$directory = Split-Path -Parent $Target
if (-not (Test-Path $directory)) { New-Item -ItemType Directory -Path $directory | Out-Null }
[System.IO.File]::WriteAllText($Target, ($buffer -join "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Output "wrote $Target"
