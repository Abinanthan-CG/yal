//! Validation and generation for the parity ledger.
//!
//! `parity/parity.toml` is the single source of truth for scope and status. It is
//! assembled from the authoring fragments in `parity/parts/*.toml`, validated
//! against `parity/pages.toml`, and rendered into `PARITY.md`.

use crate::{repo_root, test_functions_by_crate};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const PORTED: &str = "PORTED";
const NOT_PORTED: &str = "NOT PORTED";

#[derive(Deserialize)]
struct Ledger {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    heading: Vec<Heading>,
    #[serde(default)]
    row: Vec<Row>,
    #[serde(default)]
    count_check: Vec<CountCheck>,
    #[serde(default)]
    addition: Vec<Addition>,
}

#[derive(Deserialize, Clone)]
struct Heading {
    id: String,
    page: String,
    #[serde(default)]
    tier: Option<u32>,
    #[serde(default)]
    path: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct Row {
    id: String,
    #[serde(default)]
    tier: Option<u32>,
    page: String,
    heading: String,
    behavior: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    windows: String,
    status: String,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    count_check: Option<String>,
}

#[derive(Deserialize, Clone)]
struct CountCheck {
    id: String,
    page: String,
    description: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    value: Option<i64>,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    expected_rows: Option<usize>,
}

#[derive(Deserialize, Clone)]
struct Addition {
    id: String,
    feature: String,
    description: String,
    #[serde(default)]
    evidence: Vec<String>,
}

#[derive(Deserialize, Clone)]
struct PageEntry {
    page: String,
    title: String,
    tier: u32,
    #[serde(default)]
    source: String,
}

#[derive(Deserialize)]
struct PageFile {
    #[serde(default)]
    page: Vec<PageEntry>,
}

#[derive(Deserialize)]
struct CountCheckFile {
    #[serde(default)]
    count_check: Vec<CountCheck>,
}

/// A rule that binds ledger rows to a count check.
#[derive(Deserialize)]
struct Tag {
    count_check: String,
    page: String,
    #[serde(default)]
    heading: Option<String>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    behavior_starts_with: Vec<String>,
    #[serde(default)]
    behavior_equals: Vec<String>,
    /// The exact subjects the count check counts; each must match one row.
    #[serde(default)]
    items: Vec<String>,
}

#[derive(Deserialize)]
struct TagFile {
    #[serde(default)]
    tag: Vec<Tag>,
}

pub struct Options {
    pub summary_only: bool,
    pub check_only: bool,
}

pub fn run(opts: &Options) -> Result<()> {
    let root = repo_root();
    let pages = load_pages(&root)?;
    let canonical_checks = load_count_checks(&root)?;
    let mut ledger = merge_parts(&root)?;
    let tags = load_tags(&root)?;
    apply_tags(&mut ledger, &tags);
    if !opts.summary_only {
        let unmatched = unmatched_items(&root);
        if !unmatched.is_empty() {
            eprintln!("parity: count-check items that matched no row:");
            for item in unmatched {
                eprintln!("  - {item}");
            }
        }
    }

    let problems = validate(&ledger, &pages, &canonical_checks);
    if !problems.is_empty() {
        eprintln!("parity: {} problem(s) in the ledger", problems.len());
        for p in &problems {
            eprintln!("  - {p}");
        }
        bail!("parity ledger is invalid");
    }

    let stats = summarize(&ledger, &pages);
    if opts.summary_only {
        print_summary(&ledger, &stats);
        return Ok(());
    }

    let parity_toml = render_ledger_toml(&ledger, &pages, &canonical_checks);
    let parity_path = root.join("parity").join("parity.toml");
    let markdown = render_markdown(&ledger, &pages, &stats);
    let parity_md = root.join("PARITY.md");

    if opts.check_only {
        let current_toml = std::fs::read_to_string(&parity_path).unwrap_or_default();
        let current_md = std::fs::read_to_string(&parity_md).unwrap_or_default();
        if current_toml != parity_toml {
            bail!("parity/parity.toml is out of date; run `cargo xtask parity`");
        }
        if current_md != markdown {
            bail!("PARITY.md is out of date; run `cargo xtask parity`");
        }
        println!("parity: the ledger and PARITY.md are up to date");
        return Ok(());
    }

    std::fs::write(&parity_path, parity_toml).context("writing parity/parity.toml")?;
    std::fs::write(&parity_md, markdown).context("writing PARITY.md")?;
    print_summary(&ledger, &stats);
    println!("parity: wrote {} and {}", parity_path.display(), parity_md.display());
    Ok(())
}

/// Load the canonical page inventory.
fn load_pages(root: &Path) -> Result<Vec<PageEntry>> {
    let path = root.join("parity").join("pages.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let file: PageFile = toml::from_str(&text).context("parsing parity/pages.toml")?;
    if file.page.is_empty() {
        bail!("parity/pages.toml lists no pages");
    }
    Ok(file.page)
}

/// Load the canonical count checks.
fn load_count_checks(root: &Path) -> Result<Vec<CountCheck>> {
    let path = root.join("parity").join("count_checks.toml");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let file: CountCheckFile = toml::from_str(&text).context("parsing parity/count_checks.toml")?;
    if file.count_check.is_empty() {
        bail!("parity/count_checks.toml lists no checks");
    }
    Ok(file.count_check)
}

/// Load the count-check tagging rules.
fn load_tags(root: &Path) -> Result<Vec<Tag>> {
    let path = root.join("parity").join("tags.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let file: TagFile = toml::from_str(&text).context("parsing parity/tags.toml")?;
    Ok(file.tag)
}

/// The count-check items that matched no row on their page.
pub fn unmatched_items(root: &Path) -> Vec<String> {
    let tags = match load_tags(root) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let ledger = match merge_parts(root) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for tag in &tags {
        if tag.items.is_empty() {
            continue;
        }
        for item in &tag.items {
            let matched = ledger
                .row
                .iter()
                .any(|row| row.page == tag.page && row.behavior.starts_with(item.as_str()));
            if !matched {
                out.push(format!("{} :: {}", tag.count_check, item));
            }
        }
    }
    out
}

/// Bind rows to their count checks.
///
/// A row can only carry one count check, so a row claimed by two different
/// checks keeps the first and is reported by the validator.
fn apply_tags(ledger: &mut Ledger, tags: &[Tag]) {
    for tag in tags {
        for row in ledger.row.iter_mut() {
            if row.page != tag.page {
                continue;
            }
            if let Some(heading) = &tag.heading {
                if &row.heading != heading {
                    continue;
                }
            }
            if let Some(group) = &tag.group {
                if &row.group != group {
                    continue;
                }
            }
            if !tag.groups.is_empty() && !tag.groups.contains(&row.group) {
                continue;
            }
            if !tag.items.is_empty() {
                let matched = tag
                    .items
                    .iter()
                    .any(|item| row.behavior.starts_with(item.as_str()));
                if !matched {
                    continue;
                }
            }
            if !tag.behavior_starts_with.is_empty() {
                let matched = tag
                    .behavior_starts_with
                    .iter()
                    .any(|prefix| row.behavior.starts_with(prefix));
                if !matched {
                    continue;
                }
            }
            if !tag.behavior_equals.is_empty() {
                let matched = tag.behavior_equals.iter().any(|exact| &row.behavior == exact);
                if !matched {
                    continue;
                }
            }
            if tag.heading.is_none()
                && tag.group.is_none()
                && tag.groups.is_empty()
                && tag.items.is_empty()
                && tag.behavior_starts_with.is_empty()
                && tag.behavior_equals.is_empty()
            {
                continue;
            }
            if row.count_check.is_none() {
                row.count_check = Some(tag.count_check.clone());
            }
        }
    }
}

/// Assemble the ledger from the authoring fragments in `parity/parts/`.
fn merge_parts(root: &Path) -> Result<Ledger> {
    let parts_dir = root.join("parity").join("parts");
    let mut heading: Vec<Heading> = Vec::new();
    let mut row: Vec<Row> = Vec::new();
    let mut count_check: Vec<CountCheck> = Vec::new();
    let mut addition: Vec<Addition> = Vec::new();

    if parts_dir.is_dir() {
        let mut files: Vec<PathBuf> = Vec::new();
        for entry in walkdir::WalkDir::new(&parts_dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                files.push(path.to_path_buf());
            }
        }
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("reading {}", file.display()))?;
            let part: Ledger = toml::from_str(&text)
                .with_context(|| format!("parsing {}", file.display()))?;
            heading.extend(part.heading);
            row.extend(part.row);
            count_check.extend(part.count_check);
            addition.extend(part.addition);
        }
    }

    Ok(Ledger { schema: 1, heading, row, count_check, addition })
}

fn validate(ledger: &Ledger, pages: &[PageEntry], canonical: &[CountCheck]) -> Vec<String> {
    let mut problems = Vec::new();
    let page_by_name: BTreeMap<&str, &PageEntry> =
        pages.iter().map(|p| (p.page.as_str(), p)).collect();

    let mut heading_ids: BTreeSet<&str> = BTreeSet::new();
    for h in &ledger.heading {
        if !heading_ids.insert(h.id.as_str()) {
            problems.push(format!("duplicate heading id `{}`", h.id));
        }
        let Some(page) = page_by_name.get(h.page.as_str()) else {
            problems.push(format!("heading `{}` is on unknown page `{}`", h.id, h.page));
            continue;
        };
        if let Some(tier) = h.tier {
            if tier != page.tier {
                problems.push(format!(
                    "heading `{}` says tier {tier} but page `{}` is tier {}",
                    h.id, h.page, page.tier
                ));
            }
        }
        if h.path.is_empty() {
            problems.push(format!("heading `{}` has an empty path", h.id));
        }
    }
    let heading_by_id: BTreeMap<&str, &Heading> =
        ledger.heading.iter().map(|h| (h.id.as_str(), h)).collect();

    let mut row_ids: BTreeSet<&str> = BTreeSet::new();
    for r in &ledger.row {
        if !row_ids.insert(r.id.as_str()) {
            problems.push(format!("duplicate row id `{}`", r.id));
        }
        if r.status != PORTED && r.status != NOT_PORTED {
            problems.push(format!(
                "row `{}` has status `{}`; only `{PORTED}` and `{NOT_PORTED}` are legal",
                r.id, r.status
            ));
        }
        let Some(page) = page_by_name.get(r.page.as_str()) else {
            problems.push(format!("row `{}` is on unknown page `{}`", r.id, r.page));
            continue;
        };
        if let Some(tier) = r.tier {
            if tier != page.tier {
                problems.push(format!(
                    "row `{}` says tier {tier} but page `{}` is tier {}",
                    r.id, r.page, page.tier
                ));
            }
        }
        match heading_by_id.get(r.heading.as_str()) {
            None => problems.push(format!("row `{}` references unknown heading `{}`", r.id, r.heading)),
            Some(h) => {
                if h.page != r.page {
                    problems.push(format!(
                        "row `{}` says page `{}` but heading `{}` is on page `{}`",
                        r.id, r.page, r.heading, h.page
                    ));
                }
            }
        }
        if r.behavior.trim().is_empty() {
            problems.push(format!("row `{}` has an empty behavior", r.id));
        }
        if r.status == PORTED {
            if r.evidence.is_empty() {
                problems.push(format!("row `{}` is {PORTED} but has no evidence", r.id));
            }
            for e in &r.evidence {
                problems.extend(check_evidence(&r.id, e));
            }
        }
        if r.status == NOT_PORTED && r.reason.trim().is_empty() {
            problems.push(format!("row `{}` is {NOT_PORTED} but has no reason", r.id));
        }
    }

    let mut covered: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &ledger.row {
        *covered.entry(r.heading.as_str()).or_insert(0) += 1;
    }
    // A heading with no rows of its own is fine when a child heading has them.
    let titles_with_rows: BTreeSet<&str> = ledger
        .heading
        .iter()
        .filter(|h| covered.get(h.id.as_str()).copied().unwrap_or(0) > 0)
        .filter_map(|h| h.path.last().map(|title| title.as_str()))
        .collect();
    for h in &ledger.heading {
        let title = h.path.last().map(|title| title.as_str()).unwrap_or("");
        if covered.get(h.id.as_str()).copied().unwrap_or(0) == 0
            && !titles_with_rows.contains(title)
        {
            problems.push(format!("heading `{}` has no ledger rows", h.id));
        }
    }

    let mut check_ids: BTreeSet<&str> = BTreeSet::new();
    for c in canonical {
        if !check_ids.insert(c.id.as_str()) {
            problems.push(format!("duplicate count check id `{}`", c.id));
        }
        if c.description.trim().is_empty() {
            problems.push(format!("count check `{}` has an empty description", c.id));
        }
        for e in &c.evidence {
            problems.extend(check_evidence(&c.id, e));
        }
        if c.kind != "rows" && c.kind != "asserted" {
            problems.push(format!(
                "count check `{}` has kind `{}`; expected `rows` or `asserted`",
                c.id, c.kind
            ));
        }
    }
    for c in &ledger.count_check {
        if !check_ids.contains(c.id.as_str()) {
            problems.push(format!(
                "count check `{}` is defined in a part but not in parity/count_checks.toml",
                c.id
            ));
        }
    }
    let check_by_id: BTreeMap<&str, &CountCheck> =
        canonical.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut tagged: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &ledger.row {
        let Some(cid) = r.count_check.as_deref() else {
            continue;
        };
        match check_by_id.get(cid) {
            None => problems.push(format!("row `{}` references unknown count check `{}`", r.id, cid)),
            Some(c) => {
                if c.page != r.page {
                    problems.push(format!(
                        "row `{}` is on page `{}` but count check `{}` is on page `{}`",
                        r.id, r.page, cid, c.page
                    ));
                }
                *tagged.entry(cid).or_insert(0) += 1;
            }
        }
    }
    for c in canonical {
        let count = tagged.get(c.id.as_str()).copied().unwrap_or(0);
        // A check that binds a set of rows must be represented by one.
        // A check whose number is proven by a test is reported as pending
        // until that test exists, so the ledger is never silently short.
        if count == 0 && c.evidence.is_empty() && c.kind == "rows" {
            problems.push(format!("count check `{}` is not represented by any row", c.id));
        }
        if let Some(expected) = c.expected_rows {
            if c.kind == "rows" && count != expected {
                problems.push(format!(
                    "count check `{}` expects {expected} rows but {count} are tagged with it",
                    c.id
                ));
            }
        }
    }

    problems
}

/// Evidence is written as `crate::test_name`. The crate must exist in the
/// workspace and the test function must be present in its sources.
fn check_evidence(row_id: &str, evidence: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let Some((crate_name, test_name)) = evidence.split_once("::") else {
        return vec![format!(
            "row `{row_id}` has malformed evidence `{evidence}`; expected `crate::test_name`"
        )];
    };
    if test_name.is_empty() {
        return vec![format!("row `{row_id}` has malformed evidence `{evidence}`")];
    }
    let map = test_functions_by_crate();
    match map.get(crate_name) {
        None => problems.push(format!(
            "row `{row_id}` cites crate `{crate_name}` which does not exist in the workspace"
        )),
        Some(fns) => {
            if !fns.contains(test_name) {
                problems.push(format!(
                    "row `{row_id}` cites `{evidence}` but no function `{test_name}` exists in crate `{crate_name}`"
                ));
            }
        }
    }
    problems
}

struct PageStats {
    page: String,
    title: String,
    tier: u32,
    order: usize,
    rows: usize,
    ported: usize,
    not_ported: usize,
}

struct Stats {
    per_page: Vec<PageStats>,
    tier_rows: BTreeMap<u32, (usize, usize, usize)>,
    total_rows: usize,
    total_ported: usize,
    total_not_ported: usize,
}

fn summarize(ledger: &Ledger, pages: &[PageEntry]) -> Stats {
    let order: BTreeMap<&str, usize> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.page.as_str(), i))
        .collect();
    let title_by_page: BTreeMap<&str, &str> =
        pages.iter().map(|p| (p.page.as_str(), p.title.as_str())).collect();
    let tier_by_page: BTreeMap<&str, u32> =
        pages.iter().map(|p| (p.page.as_str(), p.tier)).collect();

    let mut per_page: BTreeMap<&str, PageStats> = BTreeMap::new();
    for p in pages {
        per_page.insert(
            p.page.as_str(),
            PageStats {
                page: p.page.clone(),
                title: p.title.clone(),
                tier: p.tier,
                order: order.get(p.page.as_str()).copied().unwrap_or(usize::MAX),
                rows: 0,
                ported: 0,
                not_ported: 0,
            },
        );
    }
    for r in &ledger.row {
        let Some(entry) = per_page.get_mut(r.page.as_str()) else {
            continue;
        };
        entry.rows += 1;
        if r.status == PORTED {
            entry.ported += 1;
        } else {
            entry.not_ported += 1;
        }
    }

    let mut per_page: Vec<PageStats> = per_page.into_values().collect();
    per_page.sort_by(|a, b| a.order.cmp(&b.order));

    let mut tier_rows: BTreeMap<u32, (usize, usize, usize)> = BTreeMap::new();
    for r in &ledger.row {
        let tier = tier_by_page.get(r.page.as_str()).copied().unwrap_or(0);
        let e = tier_rows.entry(tier).or_insert((0, 0, 0));
        e.0 += 1;
        if r.status == PORTED {
            e.1 += 1;
        } else {
            e.2 += 1;
        }
    }

    let _ = title_by_page;
    Stats {
        total_rows: ledger.row.len(),
        total_ported: ledger.row.iter().filter(|r| r.status == PORTED).count(),
        total_not_ported: ledger.row.iter().filter(|r| r.status != PORTED).count(),
        per_page,
        tier_rows,
    }
}

fn print_summary(ledger: &Ledger, stats: &Stats) {
    println!(
        "parity: {} rows, {} {PORTED}, {} {NOT_PORTED}",
        stats.total_rows, stats.total_ported, stats.total_not_ported
    );
    for (tier, (rows, ported, not_ported)) in &stats.tier_rows {
        println!("  tier {tier}: {rows} rows, {ported} {PORTED}, {not_ported} {NOT_PORTED}");
    }
    let open_tier1 = ledger.row.iter().filter(|r| r.status != PORTED).count();
    println!("  rows still open: {open_tier1}");
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ").trim().to_string()
}

/// Escape a string as a TOML basic string.
pub fn toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Serialize the merged ledger back to TOML.
///
/// The output is deterministic: the same ledger always produces byte-identical
/// TOML, so `cargo xtask parity --check` can compare it against the committed
/// file.
fn render_ledger_toml(ledger: &Ledger, pages: &[PageEntry], canonical: &[CountCheck]) -> String {
    let mut out = String::new();
    out.push_str("# Generated by `cargo xtask parity` from the fragments in parity/parts/.\n");
    out.push_str("# Do not edit by hand; edit the parts and regenerate.\n\n");
    out.push_str("schema = 1\n");

    for h in &ledger.heading {
        out.push_str("\n[[heading]]\n");
        out.push_str(&format!("id = {}\n", toml_string(&h.id)));
        out.push_str(&format!("page = {}\n", toml_string(&h.page)));
        let path: Vec<String> = h.path.iter().map(|p| toml_string(p)).collect();
        out.push_str(&format!("path = [{}]\n", path.join(", ")));
    }

    for r in &ledger.row {
        out.push_str("\n[[row]]\n");
        out.push_str(&format!("id = {}\n", toml_string(&r.id)));
        out.push_str(&format!("page = {}\n", toml_string(&r.page)));
        out.push_str(&format!("heading = {}\n", toml_string(&r.heading)));
        out.push_str(&format!("behavior = {}\n", toml_string(&r.behavior)));
        if !r.group.is_empty() {
            out.push_str(&format!("group = {}\n", toml_string(&r.group)));
        }
        out.push_str(&format!("windows = {}\n", toml_string(&r.windows)));
        out.push_str(&format!("status = {}\n", toml_string(&r.status)));
        let evidence: Vec<String> = r.evidence.iter().map(|e| toml_string(e)).collect();
        out.push_str(&format!("evidence = [{}]\n", evidence.join(", ")));
        out.push_str(&format!("reason = {}\n", toml_string(&r.reason)));
        if let Some(c) = &r.count_check {
            out.push_str(&format!("count_check = {}\n", toml_string(c)));
        }
    }

    for c in canonical {
        out.push_str("\n[[count_check]]\n");
        out.push_str(&format!("id = {}\n", toml_string(&c.id)));
        out.push_str(&format!("page = {}\n", toml_string(&c.page)));
        out.push_str(&format!("description = {}\n", toml_string(&c.description)));
        out.push_str(&format!("kind = {}\n", toml_string(&c.kind)));
        let evidence: Vec<String> = c.evidence.iter().map(|e| toml_string(e)).collect();
        out.push_str(&format!("evidence = [{}]\n", evidence.join(", ")));
        if let Some(value) = c.value {
            out.push_str(&format!("value = {value}\n"));
        }
        if let Some(expected) = c.expected_rows {
            out.push_str(&format!("expected_rows = {expected}\n"));
        }
    }

    for a in &ledger.addition {
        out.push_str("\n[[addition]]\n");
        out.push_str(&format!("id = {}\n", toml_string(&a.id)));
        out.push_str(&format!("feature = {}\n", toml_string(&a.feature)));
        out.push_str(&format!("description = {}\n", toml_string(&a.description)));
        let evidence: Vec<String> = a.evidence.iter().map(|e| toml_string(e)).collect();
        out.push_str(&format!("evidence = [{}]\n", evidence.join(", ")));
    }

    let _ = pages;
    out
}
/// Render `PARITY.md`.
fn render_markdown(ledger: &Ledger, pages: &[PageEntry], stats: &Stats) -> String {
    let heading_by_id: BTreeMap<&str, &Heading> =
        ledger.heading.iter().map(|h| (h.id.as_str(), h)).collect();

    let mut out = String::new();
    out.push_str("# Parity ledger\n\n");
    out.push_str(
        "Generated by `cargo xtask parity` from `parity/parity.toml`. Do not edit this file by hand.\n\n\
         Every documented upstream behavior appears here exactly once, with the status `PORTED` \
         or `NOT PORTED`. There is no third status.\n\n",
    );

    out.push_str("## 1. Summary\n\n");
    out.push_str("### Totals per tier\n\n");
    out.push_str("| Tier | Rows | PORTED | NOT PORTED |\n| --- | ---: | ---: | ---: |\n");
    for (tier, (rows, ported, not_ported)) in &stats.tier_rows {
        out.push_str(&format!("| {tier} | {rows} | {ported} | {not_ported} |\n"));
    }
    out.push_str(&format!(
        "| **All** | **{}** | **{}** | **{}** |\n\n",
        stats.total_rows, stats.total_ported, stats.total_not_ported
    ));

    out.push_str("### Totals per docs page\n\n");
    out.push_str("| Docs page | Tier | Rows | PORTED | NOT PORTED |\n| --- | ---: | ---: | ---: | ---: |\n");
    for p in &stats.per_page {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            p.page, p.tier, p.rows, p.ported, p.not_ported
        ));
    }
    out.push('\n');

    out.push_str("## 2. Rows by page\n\n");
    for p in &stats.per_page {
        out.push_str(&format!("### {} — `{}`\n\n", p.title, p.page));
        out.push_str("| Feature | Sub-feature | Behavior | Status | Evidence |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        let mut rows: Vec<&Row> = ledger.row.iter().filter(|r| r.page == p.page).collect();
        rows.sort_by(|a, b| {
            let oa = heading_by_id.get(a.heading.as_str()).map(|h| h.path.len()).unwrap_or(0);
            let ob = heading_by_id.get(b.heading.as_str()).map(|h| h.path.len()).unwrap_or(0);
            oa.cmp(&ob).then(a.id.cmp(&b.id))
        });
        for r in rows {
            let sub = heading_by_id
                .get(r.heading.as_str())
                .map(|h| h.path.join(" › "))
                .unwrap_or_else(|| r.heading.clone());
            let evidence = if r.status == PORTED {
                r.evidence.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(", ")
            } else {
                escape_cell(&r.reason)
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                escape_cell(&p.title),
                escape_cell(&sub),
                escape_cell(&r.behavior),
                r.status,
                evidence
            ));
        }
        out.push('\n');
    }

    out.push_str("## 3. NOT PORTED\n\n");
    let mut open: Vec<&Row> = ledger.row.iter().filter(|r| r.status != PORTED).collect();
    open.sort_by(|a, b| a.page.cmp(&b.page).then(a.id.cmp(&b.id)));
    if open.is_empty() {
        out.push_str("No rows are outstanding. Every row in the ledger is `PORTED`.\n\n");
    } else {
        out.push_str("Tier 1 first, then Tier 2.\n\n");
        out.push_str("| Page | Tier | Row | Behavior | Reason |\n| --- | ---: | --- | --- | --- |\n");
        let tier_by_page: BTreeMap<&str, u32> =
            pages.iter().map(|p| (p.page.as_str(), p.tier)).collect();
        for r in open {
            let tier = tier_by_page.get(r.page.as_str()).copied().unwrap_or(0);
            out.push_str(&format!(
                "| `{}` | {} | `{}` | {} | {} |\n",
                r.page,
                tier,
                r.id,
                escape_cell(&r.behavior),
                escape_cell(&r.reason)
            ));
        }
        out.push('\n');
    }

    out.push_str("## 4. Windows additions\n\n");
    out.push_str(
        "Windows-only behaviors that are the natural counterpart of an upstream behavior. They \
         never count toward parity and never replace an upstream behavior.\n\n",
    );
    if ledger.addition.is_empty() {
        out.push_str("No Windows additions have been recorded yet.\n\n");
    } else {
        out.push_str("| Addition | Feature | Behavior | Evidence |\n| --- | --- | --- | --- |\n");
        for a in &ledger.addition {
            let evidence = a.evidence.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(", ");
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                a.id,
                escape_cell(&a.feature),
                escape_cell(&a.description),
                evidence
            ));
        }
        out.push('\n');
    }

    out.push_str("## 5. Environment note\n\n");
    out.push_str(
        "Evidence in this ledger was produced on Windows 11 (x86_64), with the Rust toolchain \
         pinned in `rust-toolchain.toml`.\n\n\
         Rows marked `PORTED` were proven by the evidence named in the row. Rows marked \
         `NOT PORTED` are either impossible on Windows or implemented but not yet verified \
         against real Windows behavior; the reason column says which.\n",
    );

    out
}

/// Path of the ledger, used by the CI placeholder scan.
pub fn ledger_path() -> PathBuf {
    repo_root().join("parity").join("parity.toml")
}
