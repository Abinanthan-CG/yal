//! The emoji catalog: the entries, the sections and the skin tones.
//!
//! Mirrors upstream `Tinycast/Features/Emoji/Model/EmojiCatalog.swift`.

/// The grid sections, in display order.
///
/// The raw values are the compact codes the generated dataset uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmojiCategory {
    /// Smileys and people.
    SmileysAndPeople,
    /// Animals and nature.
    AnimalsAndNature,
    /// Food and drink.
    FoodAndDrink,
    /// Activity.
    Activity,
    /// Travel and places.
    TravelAndPlaces,
    /// Objects.
    Objects,
    /// Symbols.
    Symbols,
    /// Flags.
    Flags,
    /// Arrows.
    Arrows,
    /// Currency.
    Currency,
    /// Math.
    Math,
    /// Shapes and punctuation.
    ShapesAndPunctuation,
    /// CJK symbols.
    Cjk,
    /// Keys and technical.
    KeysAndTechnical,
}

impl EmojiCategory {
    /// Every section, in display order.
    pub const ALL: [EmojiCategory; 14] = [
        Self::SmileysAndPeople,
        Self::AnimalsAndNature,
        Self::FoodAndDrink,
        Self::Activity,
        Self::TravelAndPlaces,
        Self::Objects,
        Self::Symbols,
        Self::Flags,
        Self::Arrows,
        Self::Currency,
        Self::Math,
        Self::ShapesAndPunctuation,
        Self::Cjk,
        Self::KeysAndTechnical,
    ];

    /// The section a code names, if the code names one.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "sp" => Self::SmileysAndPeople,
            "an" => Self::AnimalsAndNature,
            "fd" => Self::FoodAndDrink,
            "ac" => Self::Activity,
            "tp" => Self::TravelAndPlaces,
            "ob" => Self::Objects,
            "sy" => Self::Symbols,
            "fl" => Self::Flags,
            "xa" => Self::Arrows,
            "xc" => Self::Currency,
            "xm" => Self::Math,
            "xs" => Self::ShapesAndPunctuation,
            "xj" => Self::Cjk,
            "xk" => Self::KeysAndTechnical,
            _ => return None,
        })
    }

    /// The compact code the dataset uses for this section.
    pub fn code(self) -> &'static str {
        match self {
            Self::SmileysAndPeople => "sp",
            Self::AnimalsAndNature => "an",
            Self::FoodAndDrink => "fd",
            Self::Activity => "ac",
            Self::TravelAndPlaces => "tp",
            Self::Objects => "ob",
            Self::Symbols => "sy",
            Self::Flags => "fl",
            Self::Arrows => "xa",
            Self::Currency => "xc",
            Self::Math => "xm",
            Self::ShapesAndPunctuation => "xs",
            Self::Cjk => "xj",
            Self::KeysAndTechnical => "xk",
        }
    }

    /// The title the section header shows.
    pub fn title(self) -> &'static str {
        match self {
            Self::SmileysAndPeople => "Smileys & People",
            Self::AnimalsAndNature => "Animals & Nature",
            Self::FoodAndDrink => "Food & Drink",
            Self::Activity => "Activity",
            Self::TravelAndPlaces => "Travel & Places",
            Self::Objects => "Objects",
            Self::Symbols => "Symbols",
            Self::Flags => "Flags",
            Self::Arrows => "Arrows",
            Self::Currency => "Currency",
            Self::Math => "Math",
            Self::ShapesAndPunctuation => "Shapes & Punctuation",
            Self::Cjk => "CJK Symbols",
            Self::KeysAndTechnical => "Keys & Technical",
        }
    }

    /// What the selected character is called in the actions menu.
    ///
    /// Unicode's later categories are symbol collections, so they say "Symbol".
    pub fn item_title(self) -> &'static str {
        match self {
            Self::Symbols
            | Self::Arrows
            | Self::Currency
            | Self::Math
            | Self::ShapesAndPunctuation
            | Self::Cjk
            | Self::KeysAndTechnical => "Symbol",
            _ => "Emoji",
        }
    }
}

/// Which section the picker shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmojiCategoryFilter {
    /// Every category, in the catalog's order.
    All,
    /// The pinned characters.
    Pinned,
    /// The characters used most.
    FrequentlyUsed,
    /// One category.
    Category(EmojiCategory),
}

impl EmojiCategoryFilter {
    /// Every filter, in the order the menu shows them.
    pub fn all() -> Vec<Self> {
        let mut filters = vec![Self::All, Self::Pinned, Self::FrequentlyUsed];
        filters.extend(EmojiCategory::ALL.into_iter().map(Self::Category));
        filters
    }

    /// The title the menu shows.
    pub fn title(self) -> String {
        match self {
            Self::All => "All Categories".to_string(),
            Self::Pinned => "Pinned".to_string(),
            Self::FrequentlyUsed => "Frequently Used".to_string(),
            Self::Category(category) => category.title().to_string(),
        }
    }
}

/// The zoom the actions menu offers: zooming in shows fewer, larger cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiGridZoom {
    /// The default number of columns.
    ActualSize,
    /// One column fewer.
    ZoomIn,
    /// One column more.
    ZoomOut,
}

/// The grid density the user can choose.
///
/// The zoom changes this for the current picker session only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiGridColumns {
    /// Six columns.
    Six,
    /// Seven columns.
    Seven,
    /// Eight columns.
    Eight,
    /// Nine columns.
    Nine,
    /// Ten columns.
    Ten,
}

impl EmojiGridColumns {
    /// Every density, from the smallest to the largest.
    pub const ALL: [EmojiGridColumns; 5] =
        [Self::Six, Self::Seven, Self::Eight, Self::Nine, Self::Ten];

    /// The density the picker starts at.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::Eight
    }

    /// How many columns this density shows.
    pub fn count(self) -> usize {
        match self {
            Self::Six => 6,
            Self::Seven => 7,
            Self::Eight => 8,
            Self::Nine => 9,
            Self::Ten => 10,
        }
    }

    /// The density a zoom asks for, when it asks for a different one.
    ///
    /// It is `None` when the zoom would change nothing: already at the default, or
    /// at six or ten columns.
    pub fn applying(self, zoom: EmojiGridZoom, default: EmojiGridColumns) -> Option<Self> {
        let next = match zoom {
            EmojiGridZoom::ActualSize => default,
            EmojiGridZoom::ZoomIn => EmojiGridColumns::ALL
                .iter()
                .copied()
                .find(|columns| columns.count() + 1 == self.count())?,
            EmojiGridZoom::ZoomOut => EmojiGridColumns::ALL
                .iter()
                .copied()
                .find(|columns| columns.count() == self.count() + 1)?,
        };
        (next != self).then_some(next)
    }
}

/// The Fitzpatrick skin tone the user prefers.
///
/// The modifier is the scalar appended to a tone-capable emoji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmojiSkinTone {
    /// No tone: the emoji's own form.
    None,
    /// Light.
    Light,
    /// Medium light.
    MediumLight,
    /// Medium.
    Medium,
    /// Medium dark.
    MediumDark,
    /// Dark.
    Dark,
}

impl EmojiSkinTone {
    /// Every tone, from the lightest to the darkest.
    pub const ALL: [EmojiSkinTone; 6] = [
        Self::None,
        Self::Light,
        Self::MediumLight,
        Self::Medium,
        Self::MediumDark,
        Self::Dark,
    ];

    /// The modifier scalar this tone stands for.
    pub fn modifier(self) -> Option<char> {
        Some(match self {
            Self::None => return None,
            Self::Light => '\u{1F3FB}',
            Self::MediumLight => '\u{1F3FC}',
            Self::Medium => '\u{1F3FD}',
            Self::MediumDark => '\u{1F3FE}',
            Self::Dark => '\u{1F3FF}',
        })
    }

    /// The title the setting shows.
    pub fn title(self) -> &'static str {
        match self {
            Self::None => "Default",
            Self::Light => "Light",
            Self::MediumLight => "Medium Light",
            Self::Medium => "Medium",
            Self::MediumDark => "Medium Dark",
            Self::Dark => "Dark",
        }
    }

    /// The tone shown in the settings, as a waving hand.
    pub fn sample(self) -> String {
        crate::catalog::apply_tone(self, "\u{1F44B}")
    }
}

/// One emoji or symbol.
///
/// The glyph is the base, untoned form, and doubles as the entry's id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiEntry {
    /// The character.
    pub glyph: String,
    /// The name it is searched by.
    pub name: String,
    /// The section it belongs to.
    pub category: EmojiCategory,
    /// Whether the character takes a skin tone.
    pub supports_skin_tone: bool,
    /// The search terms, comma-joined.
    pub keywords: String,
}

impl EmojiEntry {
    /// The entry's id, as upstream's `EmojiEntry.id`.
    pub fn id(&self) -> &str {
        &self.glyph
    }

    /// The name as the picker shows it.
    pub fn display_name(&self) -> String {
        let mut chars = self.name.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    }

    /// The character with the tone applied, when it takes one.
    pub fn display(&self, tone: EmojiSkinTone) -> String {
        if !self.supports_skin_tone || tone == EmojiSkinTone::None {
            return self.glyph.clone();
        }
        apply_tone(tone, &self.glyph)
    }

    /// The search terms, one by one.
    pub fn keyword_list(&self) -> Vec<&str> {
        if self.keywords.is_empty() {
            return Vec::new();
        }
        self.keywords.split(',').collect()
    }
}

/// The catalog's own helpers.
pub struct EmojiCatalog;

impl EmojiCatalog {
    /// Parses the generated `glyph|name|category|tone|keywords` records.
    ///
    /// A malformed line is dropped rather than guessed at.
    pub fn parse(raw: &str) -> Vec<EmojiEntry> {
        Self::parse_with_localized(raw, &[])
    }

    /// Parses the records and joins the localized keyword packs.
    ///
    /// Each pack is a `glyph|keyword,keyword` table.
    pub fn parse_with_localized(raw: &str, localized: &[String]) -> Vec<EmojiEntry> {
        let terms = localized_terms(localized);
        let mut result: Vec<EmojiEntry> = Vec::with_capacity(2200);
        for line in raw.split('\n') {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            let mut fields = line.splitn(5, '|');
            let (Some(glyph), Some(name), Some(code), Some(tone), Some(keywords)) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                continue;
            };
            let Some(category) = EmojiCategory::from_code(code) else {
                continue;
            };
            let mut keywords = keywords.to_string();
            for pack in terms.get(glyph).into_iter().flatten() {
                if !keywords.is_empty() {
                    keywords.push(',');
                }
                keywords.push_str(pack);
            }
            result.push(EmojiEntry {
                glyph: glyph.to_string(),
                name: name.to_string(),
                category,
                supports_skin_tone: tone == "1",
                keywords,
            });
        }
        result
    }

    /// Applies a tone to a character.
    ///
    /// The variation selector is stripped and the modifier appended; the modifier
    /// already forces the emoji presentation.
    pub fn apply_tone(tone: EmojiSkinTone, glyph: &str) -> String {
        apply_tone(tone, glyph)
    }
}

/// The base scalar plus the tone modifier, as upstream's
/// `EmojiCatalog.applyTone`.
pub fn apply_tone(tone: EmojiSkinTone, glyph: &str) -> String {
    let Some(modifier) = tone.modifier() else {
        return glyph.to_string();
    };
    let mut scalars: String = glyph
        .chars()
        .filter(|scalar| *scalar != '\u{FE0F}')
        .collect();
    scalars.push(modifier);
    scalars
}

/// The keyword packs, read as `glyph|keywords` tables.
fn localized_terms(packs: &[String]) -> std::collections::HashMap<String, Vec<String>> {
    let mut result: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for pack in packs {
        for line in pack.split('\n') {
            let line = line.trim_end_matches('\r');
            let Some((glyph, terms)) = line.split_once('|') else {
                continue;
            };
            result
                .entry(glyph.to_string())
                .or_default()
                .push(terms.to_string());
        }
    }
    result
}
