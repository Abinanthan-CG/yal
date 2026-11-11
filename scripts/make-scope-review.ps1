# Generates docs/scope-review.md: every open feature, grouped, for the scope review.
param(
    [string]$Root = "C:\Users\dabin\.minimax\sessions\mvs_be5482942e1f46ceab95749b01d52300\workspace"
)
$ErrorActionPreference = 'Stop'

$ledger = [System.IO.File]::ReadAllText("$Root\parity\parity.toml", [System.Text.Encoding]::UTF8)

# The heading paths, so a feature reads as a name and not as an id.
# The feature name is the last segment of the heading id: `features-calculator.units`
# is the feature "units".
$paths = @{}
foreach ($block in ($ledger -split "(?m)^\[\[heading\]\]$")) {
    if ($block -notmatch '(?m)^id = "(.+)"') { continue }
    $id = $Matches[1]
    $leaf = ($id -split '\.')[-1]
    $label = $leaf -replace '-', ' '
    $label = (Get-Culture).TextInfo.ToTitleCase($label)
    $paths[$id] = $label
}

$rows = @()
foreach ($block in ($ledger -split "(?m)^\[\[row\]\]$")) {
    if ($block -notmatch 'status = "NOT PORTED"') { continue }
    $page = ''; $head = ''; $beh = ''; $reason = ''
    if ($block -match '(?m)^page = "(.+)"') { $page = $Matches[1] }
    if ($block -match '(?m)^heading = "(.+)"') { $head = $Matches[1] }
    if ($block -match '(?m)^behavior = "(.*)"') { $beh = $Matches[1] }
    if ($block -match '(?m)^reason = "(.*)"') { $reason = $Matches[1] }
    $cut = $reason.StartsWith('Out of scope by decision')
    $rows += [pscustomobject]@{ Page = $page; Heading = $head; Behaviour = $beh; Reason = $reason; Cut = $cut }
}
$toBuild = $rows | Where-Object { -not $_.Cut }
$cutRows = $rows | Where-Object { $_.Cut }

$out = @()
$out += "# YAL: what is still to build"
$out += ""
$out += "Generated from `parity/parity.toml` on $(Get-Date -Format 'yyyy-MM-dd HH:mm')."
$out += "Every open row is here, grouped into the feature it belongs to."
$out += ""
$out += "**How to use it.** Tick the features you want. A tick means: build it, with a test"
$out += "that shows it works. A cross means: leave it out, and the row keeps its one-line reason"
$out += "in `parity/reasons.toml` - a row that is not built is never silently claimed."
$out += ""
$out += "The count in brackets is how many ledger rows the feature covers."
$out += ""
$total = $rows.Count
$out += "**$($rows.Count) rows are open: $($toBuild.Count) to build and $($cutRows.Count) out of scope by decision.**"
$out += ""

# What Windows has for each page, and what this port would do about it.
$windows = @{
    'docs/features/calculator/'   = @('BUILD', 'All of it, 1:1. The currencies come last: they need live rates.')
    'docs/ai/'                    = @('CUT', 'No model provider, no API key. Decided 2026-11-11.')
    'docs/ai/mcp/'                = @('CUT', 'With the AI.')
    'docs/ai/quick-actions/'      = @('CUT', 'With the AI.')
    'docs/launcher/commands/'     = @('BUILD', 'The built-in command catalogue, one row per command.')
    'docs/reference/shortcuts/'   = @('BUILD', 'Which key does what, per screen. The AI screens went with the AI.')
    'docs/features/window-management/' = @('BUILD', 'Switching, moving and resizing windows - real Win32.')
    'docs/reference/settings/'    = @('BUILD', 'The settings panes, their groups and their defaults.')
    'docs/features/calendar/'     = @('BUILD', 'Windows'' own calendar through WinRT, plus a command that opens the web calendar.')
    'docs/features/clipboard/'    = @('BUILD', 'All of it, 1:1: the capture loop, the watcher and the section.')
    'docs/features/file-search/'  = @('BUILD', 'The indexer and the search, over NTFS.')
    'docs/palette/'               = @('BUILD', 'The app''s own shell: the screens, the back stack, the actions menu, the keys.')
    'docs/launcher/'              = @('BUILD', 'The launcher''s own behaviour.')
    'docs/launcher/quicklinks/'   = @('BUILD', 'The store, the section and the editor.')
    'docs/reference/hotkeys/'     = @('BUILD', 'The global shortcuts you record yourself.')
    'docs/features/notes/'        = @('BUILD', 'A folder of Markdown files, as upstream.')
    'docs/features/snippets/'     = @('BUILD', 'The section, the editor and the browser.')
    'docs/reference/backup/'      = @('BUILD', 'The export and the import of your data.')
    'docs/permissions/'           = @('CUT', 'The macOS permissions have no Windows answer.')
    'docs/reference/deeplinks/'   = @('BUILD', 'Our own tinycast:// grammar.')
    'docs/features/dictation/'    = @('CUT', 'Windows'' own speech input does the job.')
    'docs/reference/import-from-raycast/' = @('BUILD', 'Low priority: the .rayconfig export can be done by hand.')
    'docs/features/window-layouts/' = @('CUT', 'The saved layouts, and the rooms that hold them, went with it.')
    'docs/features/navigation/'   = @('BUILD', 'Switching windows stays; reading a menu bar did not.')
    'docs/launcher/uninstall/'    = @('CUT', 'The in-app uninstaller.')
    'docs/'                       = @('BUILD', 'The first-run story: pick a shortcut, turn on what you want.')
    'docs/launcher/system-actions/' = @('BUILD', 'The twenty that run stay; the twelve that cannot stay open with their reason.')
    'docs/features/rooms/'        = @('CUT', 'With the window layouts.')
    'docs/extensions/'            = @('CUT', 'A macOS concept.')
    'docs/extensions/installing/' = @('CUT', 'A macOS concept.')
    'docs/extensions/compatibility/' = @('CUT', 'A macOS concept.')
    'docs/extensions/customising/' = @('CUT', 'A macOS concept.')
    'docs/reference/updates/'     = @('CUT', 'One person rebuilds it from the source.')
    'docs/features/emoji/'        = @('BUILD', 'The picker screen; the model is already done.')
    'docs/features/apple-shortcuts/' = @('CUT', 'A macOS concept.')
    'docs/install/'               = @('BUILD', 'Already done.')
    'docs/launcher/favorites/'    = @('BUILD', 'The favorites slots and the section.')
    'docs/launcher/aliases/'      = @('BUILD', 'The alias store.')
    'docs/launcher/system-settings/' = @('BUILD', 'The Windows Settings pages, opened straight from the launcher.')
    'docs/launcher/fallbacks/'    = @('BUILD', 'The fallbacks.')
    'docs/features/camera/'       = @('CUT', 'Not for now.')
    'docs/features/dictionary/'   = @('CUT', 'Define Word opens a web lookup instead.')
}

$byPage = $rows | Group-Object Page | Sort-Object { -$_.Count }

$out += "## At a glance"
$out += ""
$out += "What was decided on 2026-11-11, and what each page becomes."
$out += ""
$out += "| Page | Rows | Decision |"
$out += "| --- | --- | --- |"
foreach ($page in $byPage) {
    $name = $page.Name
    $title = if ($name -eq 'docs/') { 'The overview' } else {
        (($name -replace '^docs/', '') -replace '/$', '') -replace '/', ' > '
    }
    $title = ($title -split ' > ' | ForEach-Object {
        (Get-Culture).TextInfo.ToTitleCase($_.Replace('-', ' '))
    }) -join ' > '
    $note = if ($windows.ContainsKey($name)) { $windows[$name] } else { @('BUILD', '') }
    $build = ($page.Group | Where-Object { -not $_.Cut }).Count
    $out += "| $title | $build | **$($note[0])** - $($note[1]) |"
}
$out += ""
$out += "## To build"
$out += ""

foreach ($page in $byPage) {
    $name = $page.Name
    $title = if ($name -eq 'docs/') { 'The overview' } else {
        (($name -replace '^docs/', '') -replace '/$', '') -replace '/', ' > '
    }
    $title = ($title -split ' > ' | ForEach-Object {
        (Get-Culture).TextInfo.ToTitleCase($_.Replace('-', ' '))
    }) -join ' > '
    $open = $page.Group | Where-Object { -not $_.Cut }
    if (-not $open) { continue }
    $out += "## $title"
    $out += ""
    $out += "_$($open.Count) rows to build._"
    $out += ""
    $groups = $open | Group-Object Heading | Sort-Object { $paths[$_.Name] }
    foreach ($group in $groups) {
        $label = if ($paths.ContainsKey($group.Name)) { $paths[$group.Name] } else { $group.Name }
        $out += "- [ ] **$label** ($($group.Count))"
        $seen = @{}
        $shown = 0
        foreach ($row in $group.Group) {
            $text = $row.Behaviour
            if ($text.Length -gt 150) { $text = $text.Substring(0, 147) + '...' }
            if ($seen.ContainsKey($text)) { continue }
            $seen[$text] = $true
            $out += "    - $text"
            $shown++
            if ($shown -ge 3) {
                $left = $group.Count - $shown
                if ($left -gt 0) { $out += "    - _...and $left more in this feature._" }
                break
            }
        }
    }
    $out += ""
}

$out += "## Out of scope by decision"
$out += ""
$out += "These rows stay in the ledger as `NOT PORTED`, with the reason recorded. Delete a"
$out += "cut from `parity/status.toml` and they come back into scope."
$out += ""
foreach ($page in $byPage) {
    $cut = $page.Group | Where-Object { $_.Cut }
    if (-not $cut) { continue }
    $name = $page.Name
    $title = if ($name -eq 'docs/') { 'The overview' } else {
        (($name -replace '^docs/', '') -replace '/$', '') -replace '/', ' > '
    }
    $title = ($title -split ' > ' | ForEach-Object {
        (Get-Culture).TextInfo.ToTitleCase($_.Replace('-', ' '))
    }) -join ' > '
    $out += "- **$title** ($($cut.Count)) - $($cut[0].Reason)"
}
$out += ""

[System.IO.File]::WriteAllText("$Root\docs\scope-review.md", ($out -join "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Output "wrote docs/scope-review.md with $($byPage.Count) sections and $($rows.Count) rows"
