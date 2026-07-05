/// Shared deserialization row types for the entity-context queries.
/// These structs are used by both [`super::entity`] (for SurrealDB fetching)
/// and [`super::format`] (for output rendering).

#[derive(serde::Deserialize)]
pub(super) struct BasicRow {
    pub(super) name: String,
    pub(super) summary: Option<String>,
    pub(super) notes: Option<String>,
    #[serde(default)]
    pub(super) codex_article: Option<String>,
}

#[derive(serde::Deserialize)]
pub(super) struct PcRow {
    pub(super) name: String,
    pub(super) summary: Option<String>,
    pub(super) notes: Option<String>,
    pub(super) player_name: Option<String>,
    pub(super) character_class: Option<String>,
    pub(super) character_level: Option<i64>,
    pub(super) status: Option<String>,
    #[serde(default)]
    pub(super) codex_article: Option<String>,
}

#[derive(serde::Deserialize)]
pub(super) struct EventRow {
    pub(super) name: String,
    pub(super) summary: Option<String>,
    pub(super) notes: Option<String>,
    pub(super) date_start: Option<String>,
    pub(super) date_end: Option<String>,
    #[serde(default)]
    pub(super) codex_article: Option<String>,
}

#[derive(serde::Deserialize)]
pub(super) struct SessionRow {
    pub(super) title: String,
    pub(super) notes: Option<String>,
    pub(super) date_played: Option<String>,
    pub(super) session_number: Option<i64>,
}
