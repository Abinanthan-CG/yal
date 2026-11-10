//! Generates the parity ledger fragments from the upstream documentation.
//!
//! The documentation is the specification, and it is structured: headings,
//! tables, lists and paragraphs. Every table row, list item and paragraph is an
//! atomic behaviour, so the ledger can be derived from the pages mechanically
//! instead of summarised by hand.
//!
//! Run with `cargo xtask ledger`. The generated fragments live in
//! `parity/parts/generated/` and are committed, so a plain build never needs the
//! upstream clone.

use crate::repo_root;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A heading in a page, with the path that leads to it.
#[derive(Debug, Clone)]
struct Heading {
    title: String,
    level: usize,
    path: Vec<String>,
}

/// One atomic behaviour.
#[derive(Debug, Clone)]
struct Item {
    heading: usize,
    behavior: String,
    /// The list or category the behaviour was listed under, when there was one.
    group: Option<String>,
}

/// A parsed page.
struct Page {
    headings: Vec<Heading>,
    items: Vec<Item>,
}

/// Parse a Markdown page into headings and atomic behaviours.
fn parse_page(markdown: &str) -> Page {
    let mut headings: Vec<Heading> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut list_items: Vec<String> = Vec::new();
    let mut in_code = false;
    let mut code: Vec<String> = Vec::new();

    let flush_paragraph = |paragraph: &mut Vec<String>,
                               list_items: &mut Vec<String>,
                               headings: &Vec<Heading>,
                               items: &mut Vec<Item>| {
        if !list_items.is_empty() {
            for entry in list_items.drain(..) {
                push_item(items, headings, &entry, None);
            }
        }
        if !paragraph.is_empty() {
            let text = paragraph.join(" ");
            paragraph.clear();
            push_paragraph(items, headings, &text);
        }
    };

    let all_lines: Vec<&str> = markdown.lines().collect();
    for (line_index, raw_line) in all_lines.iter().enumerate() {
        let line = raw_line.trim_end();

        if line.trim_start().starts_with("```") {
            if in_code {
                let body = code.join("\n");
                code.clear();
                in_code = false;
                flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
                push_item(
                    &mut items,
                    &headings,
                    &format!("The documentation shows this example:\n\n```\n{body}\n```"),
                    None,
                );
            } else {
                flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
                in_code = true;
            }
            continue;
        }
        if in_code {
            code.push(line.to_string());
            continue;
        }

        // Headings
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            if level <= 4 {
                let title = clean_inline(trimmed.trim_start_matches('#').trim());
                flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
                while let Some((existing, _)) = stack.last() {
                    if *existing >= level {
                        stack.pop();
                    } else {
                        break;
                    }
                }
                let path: Vec<String> = stack.iter().map(|(_, t)| t.clone()).collect();
                stack.push((level, title.clone()));
                let path = if level == 1 {
                    Vec::new()
                } else {
                    path
                };
                headings.push(Heading {
                    title,
                    level,
                    path,
                });
                continue;
            }
        }

        // Tables
        if trimmed.starts_with('|') {
            flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
            let cells = split_table_row(trimmed);
            if is_separator_row(&cells) {
                continue;
            }
            // A row whose successor is the `---` separator is the header, and
            // is not a behaviour of its own.
            let is_header = all_lines
                .get(line_index + 1)
                .map(|next| is_separator_row(&split_table_row(next.trim())))
                .unwrap_or(false);
            if is_header {
                continue;
            }
            let (behaviours, group) = table_behaviours(&cells);
            for behaviour in behaviours {
                push_item(&mut items, &headings, &behaviour, group.clone());
            }
            continue;
        }

        // List items
        let bullet = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "));
        let numbered = trimmed
            .split_once('.')
            .filter(|(head, _)| !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()))
            .map(|(_, tail)| tail);
        if let Some(entry) = bullet.or(numbered) {
            flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
            list_items.push(entry.trim().to_string());
            continue;
        }

        // Block quotes
        if let Some(entry) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
            push_paragraph(&mut items, &headings, entry);
            continue;
        }

        if trimmed.trim().is_empty() {
            flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);
            continue;
        }

        paragraph.push(trimmed.trim().to_string());
    }
    flush_paragraph(&mut paragraph, &mut list_items, &headings, &mut items);

    Page { headings, items }
}


/// Add one behaviour under the innermost heading.
fn push_item(items: &mut Vec<Item>, headings: &[Heading], behaviour: &str, group: Option<String>) {
    let behaviour = behaviour.trim();
    if behaviour.is_empty() {
        return;
    }
    let heading = headings.len().saturating_sub(1);
    items.push(Item {
        heading,
        behavior: behaviour.to_string(),
        group,
    });
}

/// Add a paragraph, splitting a `·` list into its parts.
///
/// The split happens on the raw text, so a bold category label is recognised
/// before the emphasis markers are stripped.
fn push_paragraph(items: &mut Vec<Item>, headings: &[Heading], text: &str) {
    if !text.contains(" · ") {
        let cleaned = clean_inline(text);
        push_item(items, headings, &sentence(&cleaned), None);
        return;
    }

    let mut group: Option<String> = None;
    let mut parts: Vec<String> = Vec::new();
    for part in text.split(" · ") {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if is_bold_only(part) {
            group = Some(clean_inline(part));
            continue;
        }
        parts.push(clean_inline(part));
    }

    // A bare percentage after a "Set Volume to 0%" is shorthand for the same
    // action at another level, so it takes the same prefix and becomes its own
    // item: "Set Volume to 25%", "Set Volume to 50%" and so on.
    let mut folded: Vec<String> = Vec::new();
    for part in parts {
        if let Some(previous) = folded.last() {
            if is_bare_percentage(&part) && previous.ends_with('%') {
                let prefix = previous.trim_end_matches(|c: char| c.is_ascii_digit() || c == '%');
                folded.push(format!("{prefix}{part}"));
                continue;
            }
        }
        folded.push(part);
    }

    for part in folded {
        let behaviour = match &group {
            Some(group) => format!("{group}: {part}"),
            None => part,
        };
        push_item(items, headings, &sentence(&behaviour), group.clone());
    }
}

/// Whether a part is only a bold label, such as a group name.
fn is_bold_only(part: &str) -> bool {
    let trimmed = part.trim();
    trimmed.starts_with("**") && trimmed.ends_with("**") && trimmed.len() > 4 && !trimmed[2..trimmed.len() - 2].contains("**")
}

/// Whether a part is a bare percentage.
fn is_bare_percentage(part: &str) -> bool {
    let trimmed = part.trim();
    trimmed.len() > 1
        && trimmed.ends_with('%')
        && trimmed[..trimmed.len() - 1].chars().all(|c| c.is_ascii_digit())
}

/// The behaviours a table row describes.
///
/// A cell that holds a `·` list becomes one row per item, grouped under the
/// row's first cell, because upstream lists countable sets that way.
fn table_behaviours(cells: &[String]) -> (Vec<String>, Option<String>) {
    let cells: Vec<String> = cells
        .iter()
        .map(|cell| clean_inline(cell))
        .filter(|cell| !cell.is_empty())
        .collect();
    match cells.len() {
        0 => (Vec::new(), None),
        1 => (vec![sentence(&cells[0])], None),
        _ => {
            let first = cells[0].clone();
            let rest = cells[1..].join(" — ");
            if rest.contains(" · ") {
                let group = first.clone();
                let items: Vec<String> = rest
                    .split(" · ")
                    .map(|item| item.trim())
                    .filter(|item| !item.is_empty())
                    .map(|item| sentence(&format!("{first} — {item}")))
                    .collect();
                return (items, Some(group));
            }
            (vec![sentence(&format!("{first} — {rest}"))], None)
        }
    }
}

/// Split a Markdown table row into its cells.
fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed
        .trim_start_matches('|')
        .trim_end_matches('|')
        .trim_end_matches('|');
    inner.split('|').map(|cell| cell.trim().to_string()).collect()
}

/// Whether a table row is the `---` separator.
fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == ':'))
}

/// Remove Markdown decoration, keeping the words.
///
/// Links keep their label, `<kbd>` tags keep what they wrap, and emphasis
/// markers are dropped.
fn clean_inline(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < chars.len() {
        match chars[index] {
            '[' => {
                // A link: keep the label, drop the target.
                let mut label = String::new();
                let mut depth = 1usize;
                index += 1;
                while index < chars.len() {
                    let ch = chars[index];
                    index += 1;
                    if ch == '[' {
                        depth += 1;
                    } else if ch == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    label.push(ch);
                }
                if index < chars.len() && chars[index] == '(' {
                    let mut paren = 0usize;
                    while index < chars.len() {
                        let ch = chars[index];
                        index += 1;
                        if ch == '(' {
                            paren += 1;
                        } else if ch == ')' {
                            paren -= 1;
                            if paren == 0 {
                                break;
                            }
                        }
                    }
                }
                out.push_str(&label);
            }
            '<' => {
                // A tag such as <kbd>return</kbd>: keep what it wraps.
                let mut name = String::new();
                index += 1;
                while index < chars.len() {
                    let ch = chars[index];
                    index += 1;
                    if ch == '>' {
                        break;
                    }
                    name.push(ch);
                }
                if name.trim().eq_ignore_ascii_case("kbd") {
                    let mut inner = String::new();
                    while index < chars.len() {
                        if chars[index] == '<' {
                            let mut closing = String::new();
                            index += 1;
                            while index < chars.len() {
                                let ch = chars[index];
                                index += 1;
                                if ch == '>' {
                                    break;
                                }
                                closing.push(ch);
                            }
                            if closing.trim().eq_ignore_ascii_case("/kbd") {
                                break;
                            }
                            continue;
                        }
                        inner.push(chars[index]);
                        index += 1;
                    }
                    out.push_str(&inner);
                }
            }
            '*' | '`' | '_' => {
                // Emphasis markers carry no meaning here.
                let marker = ch_marker(&chars, index);
                if out.ends_with(marker) {
                    out.pop();
                }
                index += 1;
            }
            _ => {
                out.push(chars[index]);
                index += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The emphasis marker at `index`.
fn ch_marker(chars: &[char], index: usize) -> char {
    chars.get(index).copied().unwrap_or('*')
}
/// Make a fragment of documentation read as one sentence.
fn sentence(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = trimmed.to_string();
    let mut chars = String::new();
    for (index, ch) in out.chars().enumerate() {
        if index == 0 {
            chars.extend(ch.to_uppercase());
        } else {
            chars.push(ch);
        }
    }
    out = chars;
    if !out.ends_with('.')
        && !out.ends_with('!')
        && !out.ends_with('?')
        && !out.ends_with(':')
        && !out.ends_with('\u{2026}')
    {
        out.push('.');
    }
    out
}

/// A stable, readable identifier for a behaviour.
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 44 {
            break;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
    }
}

/// The fragment file name for a page.
fn page_slug(page: &str) -> String {
    let trimmed = page.trim_end_matches('/');
    let trimmed = trimmed.strip_prefix("docs/").unwrap_or(trimmed);
    if trimmed.is_empty() {
        return "docs-index".to_string();
    }
    trimmed.replace('/', "-")
}

/// Generate every page's fragment.
pub fn run() -> Result<()> {
    let root = repo_root();
    let pages = load_pages(&root)?;
    let out_dir = generated_dir();
    std::fs::create_dir_all(&out_dir).context("creating parity/parts/generated")?;

    let mut total_rows = 0usize;
    let mut total_headings = 0usize;
    for page in &pages {
        let source = root.join(&page.source);
        if !source.exists() {
            bail!(
                "the upstream page {} is missing at {}.\n\
                 Download it first, for example:\n  \
                 Invoke-WebRequest -Uri https://tinycast.dev/llms.md/{}/index.md -OutFile {}",
                page.page,
                source.display(),
                page.page,
                page.source
            );
        }
        let markdown = std::fs::read_to_string(&source)
            .with_context(|| format!("reading {}", source.display()))?;
        let parsed = parse_page(&markdown);
        let fragment = render_fragment(page, &parsed);
        total_rows += parsed.items.len();
        total_headings += parsed.headings.len();
        let path = out_dir.join(format!("{}.toml", page_slug(&page.page)));
        std::fs::write(&path, fragment)
            .with_context(|| format!("writing {}", path.display()))?;
        println!(
            "ledger: {:<40} {:>4} headings {:>5} rows",
            page.page,
            parsed.headings.len(),
            parsed.items.len()
        );
    }
    println!("ledger: {total_headings} headings, {total_rows} rows across {} pages", pages.len());
    Ok(())
}

fn render_fragment(page: &PageEntry, parsed: &Page) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Generated by `cargo xtask ledger` from {}.\n# Do not edit by hand; edit the generator.\n\n",
        page.source
    ));

    let mut heading_ids: Vec<String> = Vec::with_capacity(parsed.headings.len());
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let prefix = page_slug(&page.page);
    for heading in &parsed.headings {
        let mut id = format!("{prefix}.{}", slug(&heading.title));
        let counter = used.entry(id.clone()).or_insert(0);
        *counter += 1;
        if *counter > 1 {
            id = format!("{id}-{}", counter);
        }
        heading_ids.push(id);
    }

    for (index, heading) in parsed.headings.iter().enumerate() {
        if heading.level == 1 {
            continue;
        }
        out.push_str("\n[[heading]]\n");
        out.push_str(&format!("id = {}\n", crate::parity::toml_string(&heading_ids[index])));
        out.push_str(&format!("page = {}\n", crate::parity::toml_string(&page.page)));
        let path: Vec<String> = heading
            .path
            .iter()
            .map(|p| crate::parity::toml_string(p))
            .collect();
        out.push_str(&format!("path = [{}]\n", path.join(", ")));
    }

    // The content above the first subheading belongs to that subheading, so
    // the page's introduction is not lost.
    let first_subheading = parsed
        .headings
        .iter()
        .position(|h| h.level > 1)
        .unwrap_or(parsed.headings.len());

    let mut row_ids: BTreeMap<String, usize> = BTreeMap::new();
    for item in &parsed.items {
        if item.heading >= parsed.headings.len() {
            continue;
        }
        let heading_index = if parsed.headings[item.heading].level == 1 {
            first_subheading
        } else {
            item.heading
        };
        if heading_index >= parsed.headings.len() {
            continue;
        }
        let mut id = format!("{}.{}", heading_ids[item.heading], slug(&item.behavior));
        let counter = row_ids.entry(id.clone()).or_insert(0);
        *counter += 1;
        if *counter > 1 {
            id = format!("{id}-{}", counter);
        }
        out.push_str("\n[[row]]\n");
        out.push_str(&format!("id = {}\n", crate::parity::toml_string(&id)));
        out.push_str(&format!("page = {}\n", crate::parity::toml_string(&page.page)));
        out.push_str(&format!(
            "heading = {}\n",
            crate::parity::toml_string(&heading_ids[heading_index])
        ));
        out.push_str(&format!(
            "behavior = {}\n",
            crate::parity::toml_string(&item.behavior)
        ));
        if let Some(group) = &item.group {
            out.push_str(&format!("group = {}\n", crate::parity::toml_string(group)));
        }
        out.push_str(&format!(
            "windows = {}\n",
            crate::parity::toml_string(&page.windows)
        ));
        out.push_str("status = \"NOT PORTED\"\n");
        out.push_str("evidence = []\n");
        out.push_str(&format!(
            "reason = {}\n",
            crate::parity::toml_string("Not yet implemented.")
        ));
    }
    out
}

#[derive(Clone)]
struct PageEntry {
    page: String,
    source: String,
    windows: String,
}

fn load_pages(root: &Path) -> Result<Vec<PageEntry>> {
    let path = root.join("parity").join("pages.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let value: toml::Value = text.parse().context("parsing parity/pages.toml")?;
    let pages = value
        .get("page")
        .and_then(|p| p.as_array())
        .context("parity/pages.toml lists no pages")?;
    let mut out = Vec::new();
    for page in pages {
        out.push(PageEntry {
            page: page.get("page").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            source: page.get("source").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            windows: page.get("windows").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

/// Path of the generated fragments.
pub fn generated_dir() -> PathBuf {
    repo_root().join("parity").join("parts").join("generated")
}

/// Print the rows on each count check's page, so the tagging rules can be
/// written against what the generator actually produced.
pub fn report() -> Result<()> {
    let root = repo_root();
    let pages = load_pages(&root)?;
    let checks_path = root.join("parity").join("count_checks.toml");
    let checks_text = std::fs::read_to_string(&checks_path)?;
    let checks: toml::Value = checks_text.parse()?;
    let check_list = checks
        .get("count_check")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    let mut by_page: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for check in &check_list {
        let page = check.get("page").and_then(|v| v.as_str()).unwrap_or_default();
        let id = check.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        let kind = check.get("kind").and_then(|v| v.as_str()).unwrap_or_default();
        let expected = check.get("expected_rows").and_then(|v| v.as_integer());
        by_page.entry(page.to_string()).or_default().push(format!(
            "{id} [{kind}] expected={}",
            expected.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
        ));
    }

    for page in &pages {
        let Some(ids) = by_page.get(&page.page) else {
            continue;
        };
        println!("\n=== {} ===", page.page);
        for id in ids {
            println!("  check {id}");
        }
        let path = generated_dir().join(format!("{}.toml", page_slug(&page.page)));
        if !path.exists() {
            println!("  (no generated fragment)");
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let mut heading = String::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("heading = \"") {
                heading = rest.trim_end_matches('"').to_string();
            } else if let Some(rest) = line.strip_prefix("behavior = \"") {
                let behavior = rest.trim_end_matches('"');
                let short: String = behavior.chars().take(72).collect();
                println!("    [{heading}] {short}");
            }
        }
    }
    Ok(())
}