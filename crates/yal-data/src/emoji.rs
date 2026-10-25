//! The emoji dataset.
//!
//! Each entry carries a name and a set of keywords. The keywords exist so
//! that a user can find an emoji by what it means ("rocket" for 🚀) as well
//! as by what it is called ("rocket" for 🚀).

/// One emoji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emoji {
    /// The emoji character.
    pub character: String,
    /// The canonical name.
    pub name: String,
    /// Alternative search terms.
    pub keywords: Vec<String>,
}

/// The bundled dataset: character, name, keywords.
const EMOJI: &[(&str, &str, &[&str])] = &[
    ("😀", "grinning", &["face", "smile", "happy"]),
    ("😃", "smiley", &["face", "happy", "joy"]),
    ("😄", "smile", &["face", "happy"]),
    ("😁", "beaming", &["face", "grin"]),
    ("😆", "laughing", &["face", "haha"]),
    ("😅", "sweat smile", &["face", "relief"]),
    ("🤣", "rofl", &["face", "laugh", "rolling"]),
    ("😂", "joy", &["face", "tears", "laugh"]),
    ("🙂", "slight smile", &["face"]),
    ("🙃", "upside down", &["face", "silly"]),
    ("😉", "wink", &["face"]),
    ("😊", "blush", &["face", "shy"]),
    ("😇", "innocent", &["face", "angel"]),
    ("🥰", "smiling face with hearts", &["love", "affection"]),
    ("😍", "heart eyes", &["love", "crush"]),
    ("🤩", "star struck", &["excited"]),
    ("😘", "kiss", &["face", "love"]),
    ("😋", "yum", &["face", "tasty"]),
    ("😎", "sunglasses", &["cool"]),
    ("🤓", "nerd", &["geek", "glasses"]),
    ("🥳", "partying", &["celebrate", "party"]),
    ("😏", "smirk", &["face"]),
    ("😢", "crying", &["face", "sad", "tear"]),
    ("😭", "loudly crying", &["face", "sad", "tear"]),
    ("😤", "triumph", &["face", "proud"]),
    ("😡", "pouting", &["face", "angry"]),
    ("🤯", "exploding head", &["mind blown", "shock"]),
    ("😱", "screaming", &["face", "fear"]),
    ("🤔", "thinking", &["face", "hmm", "consider"]),
    ("🤫", "shushing", &["face", "quiet"]),
    ("😴", "sleeping", &["face", "zzz"]),
    ("🥱", "yawning", &["face", "tired"]),
    ("👍", "thumbs up", &["like", "approve", "yes"]),
    ("👎", "thumbs down", &["dislike", "no"]),
    ("👏", "clapping", &["applause", "praise"]),
    ("🙏", "praying", &["thanks", "please"]),
    ("💪", "muscle", &["strong", "flex"]),
    ("👋", "waving hand", &["hello", "goodbye"]),
    ("🖐️", "raised hand", &["stop", "high five"]),
    ("✌️", "victory", &["peace", "two"]),
    ("🤞", "crossed fingers", &["luck", "hope"]),
    ("🤟", "love you gesture", &["love"]),
    ("🤘", "rock on", &["metal"]),
    ("👌", "ok hand", &["okay", "perfect"]),
    ("🤌", "pinched fingers", &["italian", "small"]),
    ("🤏", "pinching hand", &["small", "tiny"]),
    ("✍️", "writing hand", &["write", "note"]),
    ("💅", "nail polish", &["manicure"]),
    ("🤳", "selfie", &["camera", "photo"]),
    ("💃", "dancing woman", &["dance"]),
    ("🕺", "dancing man", &["dance"]),
    ("👀", "eyes", &["look", "watch", "see"]),
    ("🧠", "brain", &["smart", "think"]),
    ("❤️", "red heart", &["love", "like"]),
    ("🧡", "orange heart", &["love"]),
    ("💛", "yellow heart", &["love"]),
    ("💚", "green heart", &["love"]),
    ("💙", "blue heart", &["love"]),
    ("💜", "purple heart", &["love"]),
    ("🖤", "black heart", &["love"]),
    ("🤍", "white heart", &["love"]),
    ("💔", "broken heart", &["sad", "breakup"]),
    ("❣️", "heart exclamation", &["love"]),
    ("💯", "hundred points", &["perfect", "score", "100"]),
    ("🔥", "fire", &["hot", "lit", "burn"]),
    ("✨", "sparkles", &["shiny", "magic", "new"]),
    ("⭐", "star", &["favourite", "rating"]),
    ("🌟", "glowing star", &["shine"]),
    ("⚡", "high voltage", &["lightning", "fast", "zap"]),
    ("💥", "collision", &["boom", "explosion"]),
    ("💫", "dizzy", &["star", "shoot"]),
    ("🌈", "rainbow", &["pride", "colour"]),
    ("☀️", "sun", &["weather", "bright"]),
    ("🌙", "crescent moon", &["night", "sleep"]),
    ("⛅", "sun behind cloud", &["weather", "partly cloudy"]),
    ("☁️", "cloud", &["weather"]),
    ("🌧️", "cloud with rain", &["weather", "rain"]),
    ("❄️", "snowflake", &["winter", "cold"]),
    ("🌊", "water wave", &["ocean", "sea"]),
    ("🍎", "red apple", &["fruit"]),
    ("🍌", "banana", &["fruit"]),
    ("🍕", "pizza", &["food", "slice"]),
    ("🍔", "hamburger", &["food", "burger"]),
    ("🍣", "sushi", &["food", "japanese"]),
    ("🍜", "steaming bowl", &["food", "noodles", "ramen"]),
    ("🍩", "doughnut", &["food", "sweet"]),
    ("🍪", "cookie", &["food", "sweet"]),
    ("☕", "hot beverage", &["coffee", "tea", "drink"]),
    ("🍺", "beer mug", &["drink", "beer"]),
    ("🍷", "wine glass", &["drink", "wine"]),
    ("🥂", "clinking glasses", &["celebrate", "cheers"]),
    ("🎂", "birthday cake", &["birthday", "party"]),
    ("🍰", "shortcake", &["cake", "dessert"]),
    ("🍏", "green apple", &["fruit"]),
    ("🥑", "avocado", &["food", "healthy"]),
    ("🍞", "bread", &["food"]),
    ("🧀", "cheese", &["food"]),
    ("🌮", "taco", &["food", "mexican"]),
    ("🍜", "noodles", &["food"]),
    ("🥚", "egg", &["food"]),
    ("🥞", "pancakes", &["food", "breakfast"]),
    ("🍳", "cooking", &["food", "breakfast"]),
    ("🍇", "grapes", &["fruit"]),
    ("🍓", "strawberry", &["fruit"]),
    ("🍑", "peach", &["fruit"]),
    ("🥦", "broccoli", &["food", "vegetable"]),
    ("🌽", "corn", &["food", "vegetable"]),
    ("🌶️", "hot pepper", &["food", "spicy"]),
    ("🍔", "burger", &["food"]),
    ("🥪", "sandwich", &["food", "lunch"]),
    ("🐶", "dog", &["animal", "pet"]),
    ("🐱", "cat", &["animal", "pet"]),
    ("🐭", "mouse", &["animal"]),
    ("🐹", "hamster", &["animal", "pet"]),
    ("🐰", "rabbit", &["animal", "pet"]),
    ("🦊", "fox", &["animal"]),
    ("🐻", "bear", &["animal"]),
    ("🐼", "panda", &["animal"]),
    ("🐨", "koala", &["animal"]),
    ("🐯", "tiger", &["animal"]),
    ("🦁", "lion", &["animal"]),
    ("🐮", "cow", &["animal"]),
    ("🐷", "pig", &["animal"]),
    ("🐸", "frog", &["animal"]),
    ("🐵", "monkey", &["animal"]),
    ("🐔", "chicken", &["animal", "bird"]),
    ("🐧", "penguin", &["animal", "bird"]),
    ("🐦", "bird", &["animal"]),
    ("🦆", "duck", &["animal", "bird"]),
    ("🦉", "owl", &["animal", "bird"]),
    ("🐝", "bee", &["animal", "insect"]),
    ("🦋", "butterfly", &["animal", "insect"]),
    ("🐌", "snail", &["animal"]),
    ("🐢", "turtle", &["animal"]),
    ("🐙", "octopus", &["animal", "sea"]),
    ("🐳", "spouting whale", &["animal", "sea"]),
    ("🦈", "shark", &["animal", "sea"]),
    ("🌵", "cactus", &["plant"]),
    ("🌲", "evergreen tree", &["plant", "nature"]),
    ("🌳", "deciduous tree", &["plant", "nature"]),
    ("🌴", "palm tree", &["plant", "nature"]),
    ("🌱", "seedling", &["plant", "grow"]),
    ("🌿", "herb", &["plant", "nature"]),
    ("🍀", "four leaf clover", &["plant", "luck"]),
    ("🌸", "cherry blossom", &["flower", "spring"]),
    ("🌹", "rose", &["flower"]),
    ("🌻", "sunflower", &["flower"]),
    ("🌷", "tulip", &["flower"]),
    ("⚽", "soccer ball", &["sport", "football"]),
    ("🏀", "basketball", &["sport"]),
    ("🏈", "american football", &["sport"]),
    ("⚾", "baseball", &["sport"]),
    ("🎾", "tennis", &["sport"]),
    ("🏐", "volleyball", &["sport"]),
    ("🏓", "ping pong", &["sport", "table tennis"]),
    ("🏸", "badminton", &["sport"]),
    ("🏒", "ice hockey", &["sport"]),
    ("🥊", "boxing glove", &["sport", "fight"]),
    ("🥅", "goal net", &["sport", "football"]),
    ("⛳", "flag in hole", &["sport", "golf"]),
    ("🏹", "bow and arrow", &["sport", "archery"]),
    ("🎣", "fishing pole", &["sport"]),
    ("🏆", "trophy", &["award", "win", "prize"]),
    ("🥇", "1st place medal", &["award", "gold", "win"]),
    ("🥈", "2nd place medal", &["award", "silver"]),
    ("🥉", "3rd place medal", &["award", "bronze"]),
    ("🎮", "video game", &["gaming", "controller"]),
    ("🕹️", "joystick", &["gaming"]),
    ("🎲", "game die", &["gaming", "dice"]),
    ("🧩", "puzzle piece", &["puzzle", "extension"]),
    ("🎯", "bullseye", &["target", "goal"]),
    ("🎨", "artist palette", &["art", "design", "paint"]),
    ("🎬", "clapper board", &["movie", "film", "video"]),
    ("🎤", "microphone", &["audio", "sing", "podcast"]),
    ("🎧", "headphone", &["audio", "music", "listen"]),
    ("🎸", "guitar", &["music", "instrument"]),
    ("🎹", "musical keyboard", &["music", "piano"]),
    ("🥁", "drum", &["music", "instrument"]),
    ("🎺", "trumpet", &["music", "instrument"]),
    ("🎻", "violin", &["music", "instrument"]),
    ("🎼", "musical score", &["music", "sheet"]),
    ("🚗", "automobile", &["car", "drive"]),
    ("🚕", "taxi", &["car"]),
    ("🚙", "sport utility vehicle", &["car", "suv"]),
    ("🚌", "bus", &["transport"]),
    ("🚎", "trolleybus", &["transport"]),
    ("🏎️", "racing car", &["car", "fast"]),
    ("🚓", "police car", &["car"]),
    ("🚑", "ambulance", &["emergency"]),
    ("🚒", "fire engine", &["emergency"]),
    ("🚚", "delivery truck", &["transport", "shipping"]),
    ("🚜", "tractor", &["vehicle", "farm"]),
    ("🛵", "motor scooter", &["vehicle"]),
    ("🏍️", "motorcycle", &["vehicle"]),
    ("🚲", "bicycle", &["bike", "cycle"]),
    ("🛴", "kick scooter", &["transport"]),
    ("🚂", "locomotive", &["train", "rail"]),
    ("✈️", "airplane", &["flight", "travel"]),
    ("🚀", "rocket", &["launch", "ship", "space", "fast"]),
    ("🛸", "flying saucer", &["ufo", "space"]),
    ("🚁", "helicopter", &["flight"]),
    ("⛵", "sailboat", &["boat", "sail"]),
    ("🚢", "ship", &["boat"]),
    ("⚓", "anchor", &["ship", "boat"]),
    ("🚰", "potable water", &["water", "tap"]),
    ("🚿", "shower", &["bath", "water"]),
    ("🚽", "toilet", &["bathroom"]),
    ("🧪", "test tube", &["science", "lab", "experiment"]),
    ("🔬", "microscope", &["science", "lab"]),
    ("🔭", "telescope", &["science", "space"]),
    ("📡", "satellite antenna", &["space", "signal"]),
    ("💉", "syringe", &["medical", "vaccine"]),
    ("🩺", "stethoscope", &["medical", "doctor"]),
    ("🧬", "dna", &["science", "biology", "dna"]),
    ("💊", "pill", &["medical", "medicine"]),
    ("🌐", "globe with meridians", &["world", "internet", "web"]),
    ("💻", "laptop", &["computer", "work"]),
    ("🖥️", "desktop computer", &["computer", "screen"]),
    ("⌨️", "keyboard", &["computer", "type"]),
    ("🖱️", "computer mouse", &["computer", "click"]),
    ("🖨️", "printer", &["computer", "office"]),
    ("💾", "floppy disk", &["save", "storage"]),
    ("💿", "optical disk", &["storage", "disc"]),
    ("📀", "dvd", &["storage"]),
    ("🧮", "abacus", &["calculate", "math"]),
    ("📷", "camera", &["photo", "picture"]),
    ("📹", "video camera", &["video", "record"]),
    ("🎥", "movie camera", &["video", "film"]),
    ("📞", "telephone receiver", &["phone", "call"]),
    ("☎️", "telephone", &["phone", "call"]),
    ("📱", "mobile phone", &["phone", "iphone", "android"]),
    ("📺", "television", &["tv", "screen"]),
    ("📻", "radio", &["music", "audio"]),
    ("⏰", "alarm clock", &["time", "alarm", "wake"]),
    ("⏱️", "stopwatch", &["time", "timer"]),
    ("⏳", "hourglass", &["time", "wait"]),
    ("📅", "calendar", &["date", "schedule"]),
    ("📆", "tear off calendar", &["date", "schedule"]),
    ("🗓️", "spiral calendar", &["date", "schedule"]),
    ("📇", "card index", &["contact"]),
    ("📌", "pushpin", &["pin", "important"]),
    ("📎", "paperclip", &["attachment", "file"]),
    ("📋", "clipboard", &["copy", "paste"]),
    ("📁", "file folder", &["folder", "files"]),
    ("📂", "open file folder", &["folder", "files"]),
    ("🗂️", "card index dividers", &["folder", "organise"]),
    ("📄", "page facing up", &["document", "file"]),
    ("📃", "page with curl", &["document"]),
    ("📑", "bookmark tabs", &["document", "favourite"]),
    ("📔", "notebook with decorative cover", &["notes"]),
    ("📕", "closed book", &["book", "read"]),
    ("📖", "open book", &["book", "read", "study"]),
    ("📚", "books", &["library", "read", "study"]),
    ("🔖", "bookmark", &["save", "favourite"]),
    ("🏷️", "label", &["tag", "category"]),
    ("✏️", "pencil", &["write", "edit"]),
    ("🖊️", "pen", &["write"]),
    ("🖌️", "paintbrush", &["draw", "art"]),
    ("📝", "memo", &["write", "note", "edit"]),
    ("💼", "briefcase", &["work", "business"]),
    ("🔒", "locked", &["secure", "private", "password"]),
    ("🔓", "unlocked", &["open", "unlock"]),
    ("🔑", "key", &["password", "unlock"]),
    ("🔍", "magnifying glass", &["search", "find", "zoom"]),
    ("🔎", "magnifying glass tilted left", &["search", "find"]),
    ("🔔", "bell", &["notification", "alert"]),
    ("🔕", "bell with slash", &["mute", "silent"]),
    ("📢", "loudspeaker", &["announcement"]),
    ("💡", "light bulb", &["idea", "tip"]),
    ("🗑️", "wastebasket", &["delete", "trash"]),
    ("⚙️", "gear", &["settings", "preferences", "config"]),
    ("🛠️", "hammer and wrench", &["tools", "settings", "build"]),
    ("🔧", "wrench", &["tools", "settings"]),
    ("🔨", "hammer", &["tools", "build"]),
    ("⚡", "zap", &["fast", "lightning"]),
    ("🔌", "plug", &["power", "connect"]),
    ("💻", "computer", &["code", "develop"]),
    ("📦", "package", &["box", "shipping", "release"]),
    ("📥", "inbox tray", &["download", "receive"]),
    ("📤", "outbox tray", &["upload", "send"]),
    ("🔄", "counterclockwise arrows button", &["sync", "refresh", "reload"]),
    ("🔀", "shuffle tracks button", &["random", "shuffle"]),
    ("🔁", "repeat button", &["loop", "repeat"]),
    ("⏩", "fast forward button", &["skip", "next"]),
    ("⏪", "fast reverse button", &["previous"]),
    ("⏸️", "pause button", &["media", "stop"]),
    ("⏹️", "stop button", &["media"]),
    ("🔊", "loud speaker", &["volume", "sound"]),
    ("🔉", "speaker low volume", &["volume", "quiet"]),
    ("🔇", "muted speaker", &["volume", "mute"]),
    ("🏁", "chequered flag", &["race", "finish", "winner"]),
    ("🎉", "party popper", &["celebrate", "congratulations", "party"]),
    ("🎊", "confetti ball", &["celebrate", "party"]),
    ("🥳", "partying face", &["celebrate", "party"]),
    ("🎁", "wrapped gift", &["present", "birthday"]),
    ("🎈", "balloon", &["party", "birthday"]),
    ("🎀", "ribbon", &["bow", "gift"]),
    ("🧸", "teddy bear", &["toy", "child"]),
    ("🪄", "magic wand", &["magic", "sparkle"]),
    ("💰", "money bag", &["money", "cash"]),
    ("💳", "credit card", &["money", "payment"]),
    ("💵", "dollar banknote", &["money", "cash"]),
    ("💸", "money with wings", &["money", "spend"]),
    ("💎", "gem stone", &["diamond", "jewel", "premium"]),
    ("⚖️", "balance scale", &["legal", "compare", "justice"]),
    ("🧭", "compass", &["direction", "navigate"]),
    ("🗺️", "world map", &["map", "travel"]),
    ("🏔️", "snow capped mountain", &["nature", "mountain"]),
    ("🏝️", "desert island", &["holiday", "travel"]),
    ("🚦", "vertical traffic light", &["signal"]),
    ("🚧", "construction", &["wip", "building"]),
    ("🧱", "brick", &["build", "minecraft"]),
    ("🧲", "magnet", &["attract"]),
    ("🔗", "link", &["url", "chain"]),
    ("🏥", "hospital", &["medical"]),
    ("🛏️", "bed", &["sleep", "rest"]),
    ("🛋️", "couch and lamp", &["living room", "rest"]),
    ("🧴", "lotion bottle", &["bath", "cream"]),
    ("🧹", "broom", &["clean"]),
    ("🧺", "basket", &["laundry"]),
    ("🛒", "shopping cart", &["shop", "buy", "trolley"]),
    ("🛍️", "shopping bags", &["shop", "buy"]),
    ("🆗", "ok button", &["ok", "okay"]),
    ("🆕", "new button", &["new"]),
    ("🔝", "top button", &["top", "up"]),
    ("🔙", "back button", &["back", "return"]),
    ("🔄", "repeat", &["sync"]),
    ("🆗", "ok", &["okay"]),
    ("🚫", "prohibited", &["no", "forbidden", "stop"]),
    ("⛔", "no entry", &["stop", "forbidden"]),
    ("❌", "cross mark", &["no", "wrong", "error"]),
    ("❗", "exclamation mark", &["important", "warning"]),
    ("❓", "question mark", &["help", "unknown"]),
    ("💬", "speech balloon", &["comment", "chat", "message"]),
    ("💭", "thought balloon", &["think", "idea"]),
    ("🗯️", "right anger bubble", &["speech"]),
    ("🔔", "notification", &["alert"]),
    ("✅", "check mark button", &["yes", "done", "complete", "ok"]),
    ("☑️", "check box with check", &["yes", "done"]),
    ("➕", "plus", &["add", "new"]),
    ("➖", "minus", &["subtract", "remove"]),
    ("➗", "divide", &["math", "division"]),
    ("✖️", "multiply", &["math", "times"]),
    ("♾️", "infinity", &["math", "forever"]),
    ("‼️", "double exclamation mark", &["important", "surprise"]),
    ("⁉️", "exclamation question mark", &["surprise"]),
    ("💯", "hundred", &["score"]),
    ("🔴", "red circle", &["colour", "shape", "record"]),
    ("🟠", "orange circle", &["colour", "shape"]),
    ("🟡", "yellow circle", &["colour", "shape"]),
    ("🟢", "green circle", &["colour", "shape"]),
    ("🔵", "blue circle", &["colour", "shape"]),
    ("🟣", "purple circle", &["colour", "shape"]),
    ("⚫", "black circle", &["colour", "shape"]),
    ("⚪", "white circle", &["colour", "shape"]),
    ("🔶", "orange diamond", &["colour", "shape"]),
    ("🔷", "blue diamond", &["colour", "shape"]),
    ("🔸", "small orange diamond", &["colour", "shape"]),
    ("🔹", "small blue diamond", &["colour", "shape"]),
    ("💠", "diamond with a dot", &["colour", "shape"]),
    ("🔘", "radio button", &["shape", "button"]),
    ("🔳", "white square button", &["shape", "button"]),
    ("🔲", "black square button", &["shape", "button"]),
];

/// The whole dataset.
pub fn all() -> Vec<Emoji> {
    EMOJI
        .iter()
        .map(|(character, name, keywords)| Emoji {
            character: (*character).to_string(),
            name: (*name).to_string(),
            keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
        })
        .collect()
}

/// Search the dataset by name and keyword.
///
/// An empty query returns the first `limit` entries, which is what the
/// palette shows before the user types anything.
pub fn search(query: &str, limit: usize) -> Vec<Emoji> {
    let needle = query.trim().to_lowercase();
    let all = all();

    if needle.is_empty() {
        return all.into_iter().take(limit).collect();
    }

    let mut scored: Vec<(u32, Emoji)> = all
        .into_iter()
        .filter_map(|emoji| {
            let name = emoji.name.to_lowercase();
            let score = if name == needle {
                100
            } else if name.starts_with(&needle) {
                80
            } else if name.contains(&needle) {
                60
            } else if emoji
                .keywords
                .iter()
                .any(|keyword| keyword.to_lowercase().contains(&needle))
            {
                40
            } else {
                return None;
            };
            Some((score, emoji))
        })
        .collect();

    scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, emoji)| emoji)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_is_not_empty() {
        assert!(all().len() > 100);
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<String> = all().into_iter().map(|e| e.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "emoji names must be unique");
    }

    #[test]
    fn search_finds_by_name() {
        let found = search("rocket", 5);
        assert!(!found.is_empty());
        assert_eq!(found[0].character, "🚀");
    }

    #[test]
    fn search_finds_by_keyword() {
        let found = search("pet", 10);
        assert!(found.iter().any(|e| e.character == "🐶"));
    }

    #[test]
    fn empty_query_returns_the_head_of_the_dataset() {
        let found = search("", 5);
        assert_eq!(found.len(), 5);
    }

    #[test]
    fn unmatched_query_returns_nothing() {
        assert!(search("zzzzqqq", 10).is_empty());
    }
}
