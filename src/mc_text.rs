use serde::Deserialize;

/// Structure for the Minecraft server status response.
///
/// Corresponds to the JSON response returned by the server status query.
#[derive(Debug, Deserialize)]
pub struct ServerStatus {
    /// Server version information.
    pub version: Version,

    /// Server description, usually the MOTD (Message Of The Day).
    pub description: Description,

    /// Player information: maximum allowed and currently online.
    pub players: Players,

    /// List of server mods, if any (may be absent).
    /// If no mods are present, this will be an empty array.
    #[serde(default)]
    pub mods: Vec<Mod>,

    /// Other additional fields that might be present, e.g. favicon.
    /// If absent in the response, will be None.
    #[serde(default)]
    pub favicon: Option<String>,

    /// Additional properties for future extensions.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// Server version.
#[derive(Debug, Deserialize)]
pub struct Version {
    /// Version name, e.g. "Purpur 1.21"
    pub name: String,

    /// Protocol version number, e.g. 767.
    pub protocol: i32,
}

/// Server description — usually the MOTD.
///
/// Can be either a string or a more complex JSON object,
/// so it's best represented by the `Description` type.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Description {
    /// Simple text description.
    Text(String),

    /// Complex description (text components with colors and formatting).
    Complex(serde_json::Value),
}

/// Player information.
#[derive(Debug, Deserialize)]
pub struct Players {
    /// Maximum number of players allowed on the server.
    pub max: i32,

    /// Current number of online players.
    pub online: i32,

    /// List of sample players, if present (usually empty or missing).
    #[serde(default)]
    pub sample: Vec<Player>,
}

/// Player entry, if a player list is available.
#[derive(Debug, Deserialize)]
pub struct Player {
    /// Player's name.
    pub name: String,

    /// Player's UUID.
    pub id: String,
}

/// Server mod, if a mod list is present.
#[derive(Debug, Deserialize)]
pub struct Mod {
    /// Mod identifier.
    pub id: String,

    /// Mod name.
    pub name: String,
}
