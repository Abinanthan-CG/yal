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
    $rows += [pscustomobject]@{ Page = $page; Heading = $head; Behaviour = $beh; Reason = $reason }
}

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
$out += "**$($rows.Count) rows are open across the features below.**"
$out += ""

# What Windows has for each page, and what this port would do about it.
$windows = @{
    'docs/features/calculator/'   = @('Build', 'Pure arithmetic. The currencies need live rates, so they come last.')
    'docs/ai/'                    = @('Build', 'With the user''s own key (BYOK). Apple Intelligence has no Windows answer.')
    'docs/ai/mcp/'                = @('Build', 'MCP servers are local processes; Windows runs them the same way.')
    'docs/ai/quick-actions/'      = @('Build', 'Needs the AI provider first, so it follows it.')
    'docs/launcher/commands/'     = @('Build', 'The built-in command catalogue, one row per command.')
    'docs/reference/shortcuts/'   = @('Build', 'The keyboard system: the recording, the conflicts, the chords.')
    'docs/features/window-management/' = @('Build', 'Real Win32: enumerate, move, resize, focus.')
    'docs/reference/settings/'    = @('Build', 'The settings panes, their groups and their defaults.')
    'docs/features/calendar/'     = @('Decide', 'Upstream is macOS EventKit. Windows would need Outlook or ICS; that is a real choice to make.')
    'docs/features/clipboard/'    = @('Build', 'The capture loop, the Windows clipboard watcher and the palette section.')
    'docs/features/file-search/'  = @('Build', 'The indexer and the search, over NTFS.')
    'docs/palette/'               = @('Build', 'The screens, the back stack, the actions menu and the key handling.')
    'docs/launcher/'              = @('Build', 'The launcher''s own behaviour.')
    'docs/launcher/quicklinks/'   = @('Build', 'The store, the section and the editor.')
    'docs/reference/hotkeys/'     = @('Build', 'The global shortcuts and the modifier taps.')
    'docs/features/notes/'        = @('Build', 'A folder of Markdown files, as upstream.')
    'docs/features/snippets/'     = @('Build', 'The section, the editor and the browser.')
    'docs/reference/backup/'      = @('Build', 'The export and the import of the user''s data.')
    'docs/permissions/'           = @('Leave', 'The macOS accessibility and screen-recording permissions have no Windows answer.')
    'docs/reference/deeplinks/'   = @('Build', 'Our own tinycast:// grammar.')
    'docs/features/dictation/'    = @('Decide', 'Windows has speech recognition, but not the macOS dictation API upstream uses.')
    'docs/reference/import-from-raycast/' = @('Build', 'The .rayconfig export format, once it is understood.')
    'docs/features/window-layouts/' = @('Build', 'Windows virtual desktops.')
    'docs/features/navigation/'   = @('Decide', 'Windows has UI Automation; upstream drives the macOS accessibility API.')
    'docs/launcher/uninstall/'    = @('Build', 'The in-app uninstaller.')
    'docs/'                       = @('Docs', 'The overview and the first-run story.')
    'docs/launcher/system-actions/' = @('Build', 'The twenty that run; the twelve that cannot stay open with their reason.')
    'docs/features/rooms/'        = @('Build', 'Windows virtual desktops, as the rooms.')
    'docs/extensions/'            = @('Leave', 'Raycast extensions are a macOS concept with no Windows answer.')
    'docs/extensions/installing/' = @('Leave', 'As above.')
    'docs/extensions/compatibility/' = @('Leave', 'As above.')
    'docs/extensions/customising/' = @('Leave', 'As above.')
    'docs/reference/updates/'     = @('Build', 'A check against the release feed.')
    'docs/features/emoji/'        = @('Build', 'The picker screen; the model is already done.')
    'docs/features/apple-shortcuts/' = @('Leave', 'Apple Shortcuts has no Windows answer; Windows Automations are a different thing.')
    'docs/install/'               = @('Build', 'The installer, the portable build and the checksums are done.')
    'docs/launcher/favorites/'    = @('Build', 'The favorites slots and the section.')
    'docs/launcher/aliases/'      = @('Build', 'The alias store.')
    'docs/launcher/system-settings/' = @('Build', 'The settings the launcher itself reads.')
    'docs/launcher/fallbacks/'    = @('Build', 'The fallbacks.')
    'docs/features/camera/'       = @('Decide', 'Windows has a camera, but not the way upstream reads one.')
    'docs/features/dictionary/'   = @('Decide', 'Windows has no built-in dictionary to look a word up in.')
}

$byPage = $rows | Group-Object Page | Sort-Object { -$_.Count }

$out += "## At a glance"
$out += ""
$out += "What Windows has for each page, and what this port would do about it."
$out += ""
$out += "| Page | Rows | On Windows |"
$out += "| --- | --- | --- |"
foreach ($page in $byPage) {
    $name = $page.Name
    $title = if ($name -eq 'docs/') { 'The overview' } else {
        (($name -replace '^docs/', '') -replace '/$', '') -replace '/', ' > '
    }
    $title = ($title -split ' > ' | ForEach-Object {
        (Get-Culture).TextInfo.ToTitleCase($_.Replace('-', ' '))
    }) -join ' > '
    $note = if ($windows.ContainsKey($name)) { $windows[$name] } else { @('Decide', 'No note yet.') }
    $out += "| $title | $($page.Count) | **$($note[0])** - $($note[1]) |"
}
$out += ""

foreach ($page in $byPage) {
    $name = $page.Name
    $title = if ($name -eq 'docs/') { 'The overview' } else {
        (($name -replace '^docs/', '') -replace '/$', '') -replace '/', ' > '
    }
    $title = ($title -split ' > ' | ForEach-Object {
        (Get-Culture).TextInfo.ToTitleCase($_.Replace('-', ' '))
    }) -join ' > '
    $out += "## $title"
    $out += ""
    $out += "_$($page.Count) open rows._"
    $out += ""
    $groups = $page.Group | Group-Object Heading | Sort-Object { $paths[$_.Name] }
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

[System.IO.File]::WriteAllText("$Root\docs\scope-review.md", ($out -join "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Output "wrote docs/scope-review.md with $($byPage.Count) sections and $($rows.Count) rows"
