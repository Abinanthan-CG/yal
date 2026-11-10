//! `tinycast://` link parsing.
//!
//! Ported from upstream's `Features/DeepLinks/Model/DeepLink.swift` and
//! `DeepLinkInput.swift`. The rules that matter:
//!
//! * A link either names a target and carries valid input, or it runs nothing.
//! * The `extensions` host belongs to Raycast extensions, so built-in parsing
//!   must never claim it.
//! * A parameter over the size limit fails the whole link rather than running
//!   without it.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// What a link carries besides its target: text, field values and an editor's contents.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeepLinkInput {
    pub fallback_text: Option<String>,
    pub arguments: Option<Arguments>,
    pub context: Option<Context>,
}

/// The `arguments` parameter: named field values or positional ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arguments {
    Named(BTreeMap<String, String>),
    Positional(Vec<String>),
}

/// The `context` parameter: one object's fields, or a list of objects to add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    Fields(BTreeMap<String, String>),
    List(Vec<BTreeMap<String, String>>),
}

impl DeepLinkInput {
    /// Bytes, not characters: one character can carry any number of combining marks.
    pub const MAXIMUM_PARAMETER_BYTES: usize = 65_536;

    /// How long the summary shown before a link is allowed to run may be.
    const SUMMARY_LENGTH: usize = 160;

    /// The empty input.
    pub fn none() -> Self {
        Self::default()
    }

    /// Build the input from raw query items.
    ///
    /// `None` when a parameter is malformed or oversized: the link fails rather
    /// than run without it.
    pub fn from_query_items(items: &[QueryItem]) -> Option<Self> {
        let mut input = DeepLinkInput::none();
        for item in items {
            let Some(value) = item.value.as_deref() else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            if value.len() > Self::MAXIMUM_PARAMETER_BYTES {
                return None;
            }
            match item.name.to_ascii_lowercase().as_str() {
                "fallbacktext" => input.fallback_text = Some(value.to_string()),
                "arguments" => input.arguments = Some(Arguments::from_json(value)?),
                "context" => input.context = Some(Context::from_json(value)?),
                _ => continue,
            }
        }
        Some(input)
    }

    /// Whether the link carries anything at all.
    pub fn is_empty(&self) -> bool {
        self.fallback_text.is_none() && self.arguments.is_none() && self.context.is_none()
    }

    /// The context fields, when the context is a single object's fields.
    pub fn context_fields(&self) -> Option<&BTreeMap<String, String>> {
        match &self.context {
            Some(Context::Fields(fields)) => Some(fields),
            _ => None,
        }
    }

    /// The input as a reader checks it before letting a link run with it; long input is cut.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(text) = &self.fallback_text {
            parts.push(text.clone());
        }
        if let Some(arguments) = &self.arguments {
            parts.push(arguments.to_json());
        }
        if let Some(context) = &self.context {
            parts.push(context.to_json());
        }
        let full = parts.join(" · ");
        if full.chars().count() > Self::SUMMARY_LENGTH {
            let cut: String = full.chars().take(Self::SUMMARY_LENGTH).collect();
            format!("{cut}…")
        } else {
            full
        }
    }

    /// Named values fill the first field of that name; with no arguments, `fallbackText` the first.
    pub fn field_values(
        &self,
        names: &[String],
        field_id: impl Fn(usize) -> String,
    ) -> BTreeMap<String, String> {
        let source = match &self.arguments {
            Some(arguments) => arguments.clone(),
            None => match &self.fallback_text {
                Some(text) => Arguments::Positional(vec![text.clone()]),
                None => return BTreeMap::new(),
            },
        };
        match source {
            Arguments::Positional(values) => {
                let mut filled = BTreeMap::new();
                for (index, value) in values.iter().enumerate() {
                    if index >= names.len() {
                        break;
                    }
                    // The first value wins when a field is named twice.
                    filled.entry(field_id(index)).or_insert_with(|| value.clone());
                }
                filled
            }
            Arguments::Named(values) => {
                let mut filled = BTreeMap::new();
                let mut claimed: Vec<&str> = Vec::new();
                for (index, name) in names.iter().enumerate() {
                    if claimed.contains(&name.as_str()) {
                        continue;
                    }
                    let Some(value) = values.get(name) else {
                        continue;
                    };
                    claimed.push(name);
                    filled.insert(field_id(index), value.clone());
                }
                filled
            }
        }
    }

    /// The query items that carry this input, percent-encoded with RFC 3986's
    /// unreserved set so the link reads back exactly.
    pub fn percent_encoded_query_items(&self) -> Vec<QueryItem> {
        let mut items = Vec::new();
        if let Some(text) = &self.fallback_text {
            items.push(QueryItem {
                name: "fallbackText".to_string(),
                value: Some(text.clone()),
            });
        }
        if let Some(arguments) = &self.arguments {
            items.push(QueryItem {
                name: "arguments".to_string(),
                value: Some(arguments.to_json()),
            });
        }
        if let Some(context) = &self.context {
            items.push(QueryItem {
                name: "context".to_string(),
                value: Some(context.to_json()),
            });
        }
        items
    }
}

impl Arguments {
    /// Parse the `arguments` parameter.
    ///
    /// A JSON object is named arguments; a JSON array is positional ones.
    pub fn from_json(raw: &str) -> Option<Arguments> {
        let value: Value = serde_json::from_str(raw).ok()?;
        match value {
            Value::Object(map) => Some(Arguments::Named(texts_of_object(&map)?)),
            Value::Array(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(scalar_text(&item)?);
                }
                Some(Arguments::Positional(values))
            }
            _ => None,
        }
    }

    /// The JSON form, with sorted keys and unescaped slashes.
    pub fn to_json(&self) -> String {
        match self {
            Arguments::Named(values) => encode_object(values),
            Arguments::Positional(values) => {
                let array: Vec<Value> = values.iter().map(|v| Value::String(v.clone())).collect();
                serde_json::to_string(&array).unwrap_or_default()
            }
        }
    }
}

impl Context {
    /// Parse the `context` parameter.
    ///
    /// A JSON object fills one editor; a JSON array of objects is a batch.
    pub fn from_json(raw: &str) -> Option<Context> {
        let value: Value = serde_json::from_str(raw).ok()?;
        match value {
            Value::Object(map) => Some(Context::Fields(texts_of_object(&map)?)),
            Value::Array(items) => {
                let mut objects = Vec::with_capacity(items.len());
                for item in items {
                    let Value::Object(map) = item else {
                        return None;
                    };
                    objects.push(texts_of_object(&map)?);
                }
                Some(Context::List(objects))
            }
            _ => None,
        }
    }

    /// The JSON form, with sorted keys and unescaped slashes.
    pub fn to_json(&self) -> String {
        match self {
            Context::Fields(values) => encode_object(values),
            Context::List(values) => {
                let array: Vec<Value> = values
                    .iter()
                    .map(|fields| {
                        let mut map = Map::new();
                        for (key, value) in fields {
                            map.insert(key.clone(), Value::String(value.clone()));
                        }
                        Value::Object(map)
                    })
                    .collect();
                serde_json::to_string(&array).unwrap_or_default()
            }
        }
    }
}

/// The text a field would hold for a JSON scalar.
///
/// Nested objects and arrays have none, so a half-read object never passes for
/// the whole one.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some(String::new()),
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(if *flag { "true".into() } else { "false".into() }),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                Some(integer.to_string())
            } else if let Some(unsigned) = number.as_u64() {
                Some(unsigned.to_string())
            } else {
                number.as_f64().map(swift_double_text)
            }
        }
        Value::Array(_) | Value::Object(_) => None,
    }
}

/// Format a double the way Swift's `String(_: Double)` does.
///
/// Swift always shows a fractional part, so `2.0` reads as `2.0` and not `2`.
fn swift_double_text(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let text = value.to_string();
    if text.contains('.') || text.contains('e') || text.contains('E') {
        text
    } else {
        format!("{text}.0")
    }
}

/// Every value of an object as text, or `None` when any value was nested.
fn texts_of_object(map: &Map<String, Value>) -> Option<BTreeMap<String, String>> {
    let mut texts = BTreeMap::new();
    for (key, value) in map {
        texts.insert(key.clone(), scalar_text(value)?);
    }
    Some(texts)
}

/// Encode a map as JSON with sorted keys and unescaped slashes.
fn encode_object(values: &BTreeMap<String, String>) -> String {
    let mut map = Map::new();
    for (key, value) in values {
        map.insert(key.clone(), Value::String(value.clone()));
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_default()
}

/// One query parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryItem {
    pub name: String,
    pub value: Option<String>,
}

/// The target a `tinycast://` link names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLink {
    /// Open the launcher, optionally with text already typed.
    Launcher { fallback_text: Option<String> },
    /// Run a launcher row, as its shortcut would.
    Entry { entry_id: String, input: DeepLinkInput },
    /// Open a Settings pane.
    Settings { slug: String },
    /// Open a note by file name, which is all a note is known by.
    Note { name: String },
    /// Open an AI chat by its identifier.
    Chat { id: String },
    /// Resize a window to a named size.
    WindowSize(WindowSize),
}

impl DeepLink {
    /// The host extension links own, so built-in parsing must never claim it.
    pub const EXTENSIONS_HOST: &'static str = "extensions";
    /// The host that opens the launcher.
    pub const SEARCH_HOST: &'static str = "search";
    /// The host that resizes a window.
    pub const WINDOW_SIZE_HOST: &'static str = "window-size";
    const NOTE_HOST: &'static str = "note";
    const CHAT_HOST: &'static str = "chat";

    /// Parse a link.
    ///
    /// `None` when the scheme is wrong, the host is unknown, or the input is
    /// malformed: a broken link runs nothing.
    pub fn parse(url: &str, scheme: &str) -> Option<DeepLink> {
        let url = crate::url::ParsedUrl::parse(url)?;
        if !url.scheme.eq_ignore_ascii_case(scheme) {
            return None;
        }
        let input = DeepLinkInput::from_query_items(&url.query_items)?;
        Self::link(&url.host.to_ascii_lowercase(), &url.path, input)
    }

    /// The link a launcher row's id makes.
    pub fn url_for_entry(entry_id: &str, scheme: &str, input: &DeepLinkInput) -> Option<String> {
        let colon = entry_id.find(':')?;
        crate::entry::action_for_entry_id(entry_id)?;
        Some(Self::build_url(
            scheme,
            &entry_id[..colon],
            &entry_id[colon + 1..],
            input,
        ))
    }

    /// The link a note makes.
    pub fn url_for_note(name: &str, scheme: &str) -> Option<String> {
        Some(Self::build_url(scheme, Self::NOTE_HOST, name, &DeepLinkInput::none()))
    }

    /// The link a chat makes.
    pub fn url_for_chat(id: &str, scheme: &str) -> Option<String> {
        Some(Self::build_url(
            scheme,
            Self::CHAT_HOST,
            &id.to_ascii_lowercase(),
            &DeepLinkInput::none(),
        ))
    }

    fn build_url(scheme: &str, host: &str, path: &str, input: &DeepLinkInput) -> String {
        let mut url = format!("{scheme}://{host}/{path}");
        let items = input.percent_encoded_query_items();
        if !items.is_empty() {
            let query = items
                .iter()
                .map(|item| {
                    let value = item.value.clone().unwrap_or_default();
                    format!(
                        "{}={}",
                        crate::url::percent_encode(&item.name),
                        crate::url::percent_encode(&value)
                    )
                })
                .collect::<Vec<_>>()
                .join("&");
            url.push('?');
            url.push_str(&query);
        }
        url
    }

    fn link(host: &str, path: &str, input: DeepLinkInput) -> Option<DeepLink> {
        match (host, path.is_empty()) {
            ("", true) | (Self::SEARCH_HOST, true) => Some(DeepLink::Launcher {
                fallback_text: input.fallback_text.clone(),
            }),
            ("settings", false) => {
                if path.contains('/') {
                    return None;
                }
                Some(DeepLink::Settings {
                    slug: path.to_ascii_lowercase(),
                })
            }
            (Self::NOTE_HOST, false) => {
                if path.contains('/') || path == "." || path == ".." {
                    return None;
                }
                Some(DeepLink::Note {
                    name: path.to_string(),
                })
            }
            (Self::CHAT_HOST, false) => {
                if !crate::ids::is_uuid(path) {
                    return None;
                }
                Some(DeepLink::Chat {
                    id: path.to_ascii_lowercase(),
                })
            }
            (Self::WINDOW_SIZE_HOST, true) => {
                let Some(Arguments::Named(fields)) = input.arguments.as_ref() else {
                    return None;
                };
                Some(DeepLink::WindowSize(WindowSize::from_link_fields(fields)?))
            }
            (Self::EXTENSIONS_HOST, _) | (_, true) => None,
            _ => {
                let entry_id = format!("{host}:{path}");
                crate::entry::action_for_entry_id(&entry_id)?;
                Some(DeepLink::Entry { entry_id, input })
            }
        }
    }
}

/// A window size a link asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSize {
    pub width: Dimension,
    pub height: Dimension,
    pub anchor: WindowLayoutAnchor,
    pub offset: Offset,
}

/// A length, in device-independent pixels or as a share of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimension {
    pub value: i32,
    pub unit: DimensionUnit,
}

/// The unit a dimension is measured in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionUnit {
    Points,
    Percent,
}

impl DimensionUnit {
    /// The accepted range; wider than any display for points, so a real size is
    /// never clipped at authoring.
    pub fn range(self) -> std::ops::RangeInclusive<i32> {
        match self {
            DimensionUnit::Points => 1..=16_000,
            DimensionUnit::Percent => 1..=100,
        }
    }
}

impl Dimension {
    /// Parse a dimension from a link value.
    ///
    /// `1200` is points and `60%` a share of the screen; out of range is
    /// refused, never clamped.
    pub fn from_link_value(link_value: &str) -> Option<Dimension> {
        let trimmed = link_value.trim();
        let unit = if trimmed.ends_with('%') {
            DimensionUnit::Percent
        } else {
            DimensionUnit::Points
        };
        let digits = if unit == DimensionUnit::Percent {
            &trimmed[..trimmed.len() - 1]
        } else {
            trimmed
        };
        let value: i32 = digits.parse().ok()?;
        if !unit.range().contains(&value) {
            return None;
        }
        Some(Dimension { value, unit })
    }
}

/// Where a size is placed on the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLayoutAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl WindowLayoutAnchor {
    /// The stored spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            WindowLayoutAnchor::TopLeft => "top-left",
            WindowLayoutAnchor::Top => "top",
            WindowLayoutAnchor::TopRight => "top-right",
            WindowLayoutAnchor::Left => "left",
            WindowLayoutAnchor::Center => "center",
            WindowLayoutAnchor::Right => "right",
            WindowLayoutAnchor::BottomLeft => "bottom-left",
            WindowLayoutAnchor::Bottom => "bottom",
            WindowLayoutAnchor::BottomRight => "bottom-right",
        }
    }

    fn parse(value: &str) -> Option<WindowLayoutAnchor> {
        Some(match value {
            "top-left" => WindowLayoutAnchor::TopLeft,
            "top" => WindowLayoutAnchor::Top,
            "top-right" => WindowLayoutAnchor::TopRight,
            "left" => WindowLayoutAnchor::Left,
            "center" => WindowLayoutAnchor::Center,
            "right" => WindowLayoutAnchor::Right,
            "bottom-left" => WindowLayoutAnchor::BottomLeft,
            "bottom" => WindowLayoutAnchor::Bottom,
            "bottom-right" => WindowLayoutAnchor::BottomRight,
            _ => return None,
        })
    }
}

/// A nudge applied on top of the anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offset {
    pub x: i32,
    pub y: i32,
}

impl Offset {
    /// The accepted range.
    pub const RANGE: std::ops::RangeInclusive<i32> = -4000..=4000;

    fn from_link_value(value: &str) -> Option<Offset> {
        let x: i32 = value.parse().ok()?;
        let y: i32 = value.parse().ok().unwrap_or(0);
        if !Self::RANGE.contains(&x) {
            return None;
        }
        Some(Offset { x, y })
    }
}

impl WindowSize {
    /// Build a size from a link's named fields.
    pub fn from_link_fields(fields: &BTreeMap<String, String>) -> Option<WindowSize> {
        let width = Dimension::from_link_value(fields.get("width")?)?;
        let height = Dimension::from_link_value(fields.get("height")?)?;
        let anchor = match fields.get("anchor") {
            None => WindowLayoutAnchor::Center,
            Some(value) => WindowLayoutAnchor::parse(&value.to_ascii_lowercase())?,
        };
        let offset = match fields.get("x") {
            None => Offset { x: 0, y: 0 },
            Some(value) => Offset::from_link_value(value)?,
        };
        let offset = match fields.get("y") {
            None => offset,
            Some(value) => {
                let y: i32 = value.parse().ok()?;
                if !Offset::RANGE.contains(&y) {
                    return None;
                }
                Offset { x: offset.x, y }
            }
        };
        Some(WindowSize {
            width,
            height,
            anchor,
            offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEME: &str = "tinycast";

    fn parse(url: &str) -> Option<DeepLink> {
        DeepLink::parse(url, SCHEME)
    }

    #[test]
    fn a_bare_link_opens_the_launcher() {
        assert_eq!(
            parse("tinycast://"),
            Some(DeepLink::Launcher { fallback_text: None })
        );
        assert_eq!(
            parse("tinycast://search"),
            Some(DeepLink::Launcher { fallback_text: None })
        );
    }

    #[test]
    fn fallback_text_reaches_the_launcher() {
        let link = parse("tinycast://?fallbackText=hello").unwrap();
        assert_eq!(
            link,
            DeepLink::Launcher {
                fallback_text: Some("hello".to_string())
            }
        );
    }

    #[test]
    fn the_scheme_must_match() {
        assert!(parse("https://search").is_none());
    }

    #[test]
    fn the_extensions_host_is_never_claimed() {
        assert!(parse("tinycast://extensions/anything").is_none());
    }

    #[test]
    fn settings_links_name_a_single_pane() {
        assert_eq!(
            parse("tinycast://settings/general"),
            Some(DeepLink::Settings {
                slug: "general".to_string()
            })
        );
        assert!(parse("tinycast://settings/general/advanced").is_none());
    }

    #[test]
    fn note_links_name_a_file() {
        assert_eq!(
            parse("tinycast://note/Shopping.md"),
            Some(DeepLink::Note {
                name: "Shopping.md".to_string()
            })
        );
        assert!(parse("tinycast://note/folder/Shopping.md").is_none());
        assert!(parse("tinycast://note/.").is_none());
        assert!(parse("tinycast://note/..").is_none());
    }

    #[test]
    fn chat_links_need_a_uuid() {
        let id = "3f2504e0-4f89-11d3-9a0c-0305e82c3301";
        assert_eq!(
            parse(&format!("tinycast://chat/{id}")),
            Some(DeepLink::Chat { id: id.to_string() })
        );
        assert!(parse("tinycast://chat/not-a-uuid").is_none());
    }

    #[test]
    fn entry_links_name_a_known_row() {
        let link = parse("tinycast://command/clipboard-history").unwrap();
        assert_eq!(
            link,
            DeepLink::Entry {
                entry_id: "command:clipboard-history".to_string(),
                input: DeepLinkInput::none(),
            }
        );
        assert!(parse("tinycast://command/not-a-command").is_none());
    }

    #[test]
    fn an_oversized_parameter_fails_the_link() {
        let long = "a".repeat(DeepLinkInput::MAXIMUM_PARAMETER_BYTES + 1);
        let url = format!("tinycast://?fallbackText={long}");
        assert!(parse(&url).is_none());
    }

    #[test]
    fn a_malformed_argument_fails_the_link() {
        assert!(parse("tinycast://?arguments=not-json").is_none());
        assert!(parse("tinycast://?context=not-json").is_none());
    }

    #[test]
    fn arguments_accept_an_object_or_an_array() {
        let named = parse("tinycast://?arguments=%7B%22a%22%3A%221%22%7D").unwrap();
        let DeepLink::Launcher { fallback_text } = named else {
            panic!("expected a launcher link");
        };
        assert_eq!(fallback_text, None);

        let input = DeepLinkInput::from_query_items(&[QueryItem {
            name: "arguments".to_string(),
            value: Some(r#"{"a":"1","b":"2"}"#.to_string()),
        }])
        .unwrap();
        assert_eq!(
            input.arguments,
            Some(Arguments::Named(
                [("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())]
                    .into_iter()
                    .collect()
            ))
        );

        let positional = DeepLinkInput::from_query_items(&[QueryItem {
            name: "arguments".to_string(),
            value: Some(r#"["one","two"]"#.to_string()),
        }])
        .unwrap();
        assert_eq!(
            positional.arguments,
            Some(Arguments::Positional(vec![
                "one".to_string(),
                "two".to_string()
            ]))
        );
    }

    #[test]
    fn a_nested_argument_fails_the_link() {
        let input = DeepLinkInput::from_query_items(&[QueryItem {
            name: "arguments".to_string(),
            value: Some(r#"{"a":{"b":"1"}}"#.to_string()),
        }]);
        assert!(input.is_none());
    }

    #[test]
    fn context_accepts_one_object_or_a_list() {
        let fields = DeepLinkInput::from_query_items(&[QueryItem {
            name: "context".to_string(),
            value: Some(r#"{"title":"Hi"}"#.to_string()),
        }])
        .unwrap();
        assert_eq!(
            fields.context,
            Some(Context::Fields(
                [("title".to_string(), "Hi".to_string())].into_iter().collect()
            ))
        );

        let list = DeepLinkInput::from_query_items(&[QueryItem {
            name: "context".to_string(),
            value: Some(r#"[{"title":"One"},{"title":"Two"}]"#.to_string()),
        }])
        .unwrap();
        assert!(matches!(list.context, Some(Context::List(objects)) if objects.len() == 2));

        let mixed = DeepLinkInput::from_query_items(&[QueryItem {
            name: "context".to_string(),
            value: Some(r#"[{"title":"One"},2]"#.to_string()),
        }]);
        assert!(mixed.is_none());
    }

    #[test]
    fn scalars_become_the_text_a_field_would_hold() {
        let input = DeepLinkInput::from_query_items(&[QueryItem {
            name: "arguments".to_string(),
            value: Some(r#"{"a":1,"b":true,"c":null,"d":2.5}"#.to_string()),
        }])
        .unwrap();
        let Some(Arguments::Named(values)) = input.arguments else {
            panic!("expected named arguments");
        };
        assert_eq!(values.get("a").map(String::as_str), Some("1"));
        assert_eq!(values.get("b").map(String::as_str), Some("true"));
        assert_eq!(values.get("c").map(String::as_str), Some(""));
        assert_eq!(values.get("d").map(String::as_str), Some("2.5"));
    }

    #[test]
    fn field_values_fill_the_first_field_of_that_name() {
        let input = DeepLinkInput::from_query_items(&[QueryItem {
            name: "arguments".to_string(),
            value: Some(r#"{"title":"Hi"}"#.to_string()),
        }])
        .unwrap();
        let names = vec!["title".to_string(), "body".to_string()];
        let filled = input.field_values(&names, |index| format!("field{index}"));
        assert_eq!(filled.get("field0").map(String::as_str), Some("Hi"));
        assert_eq!(filled.len(), 1);
    }

    #[test]
    fn fallback_text_fills_the_first_field() {
        let input = DeepLinkInput::from_query_items(&[QueryItem {
            name: "fallbackText".to_string(),
            value: Some("hello".to_string()),
        }])
        .unwrap();
        let names = vec!["title".to_string(), "body".to_string()];
        let filled = input.field_values(&names, |index| format!("field{index}"));
        assert_eq!(filled.get("field0").map(String::as_str), Some("hello"));
    }

    #[test]
    fn a_window_size_link_needs_named_arguments() {
        let link = parse(
            "tinycast://window-size?arguments=%7B%22width%22%3A%2260%25%22%2C%22height%22%3A%2240%25%22%7D",
        )
        .unwrap();
        let DeepLink::WindowSize(size) = link else {
            panic!("expected a window size link");
        };
        assert_eq!(size.width.value, 60);
        assert_eq!(size.width.unit, DimensionUnit::Percent);
        assert_eq!(size.height.value, 40);
        assert_eq!(size.anchor, WindowLayoutAnchor::Center);
        assert_eq!(size.offset, Offset { x: 0, y: 0 });
    }

    #[test]
    fn a_window_size_link_refuses_an_out_of_range_dimension() {
        let input = DeepLinkInput::from_query_items(&[QueryItem {
            name: "arguments".to_string(),
            value: Some(r#"{"width":"0%","height":"40%"}"#.to_string()),
        }]);
        assert!(input.is_some());
        let Some(Arguments::Named(fields)) = input.unwrap().arguments else {
            panic!("expected named arguments");
        };
        assert!(WindowSize::from_link_fields(&fields).is_none());
    }

    #[test]
    fn a_link_round_trips_through_its_own_form() {
        let original = "tinycast://note/Shopping.md";
        let parsed = parse(original).unwrap();
        let DeepLink::Note { name } = parsed else {
            panic!("expected a note link");
        };
        assert_eq!(DeepLink::url_for_note(&name, SCHEME).unwrap(), original);
    }

    #[test]
    fn the_summary_is_capped() {
        let long = "a".repeat(400);
        let input = DeepLinkInput {
            fallback_text: Some(long),
            ..DeepLinkInput::none()
        };
        let summary = input.summary();
        assert_eq!(summary.chars().count(), 161);
        assert!(summary.ends_with('…'));
    }
}
