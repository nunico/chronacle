//! Record ↔ vault-key mapping.
//!
//! A key is a POSIX-style, `/`-separated path relative to the vault root. It is
//! **derived** from the record and never authoritative: identity is the
//! frontmatter `id`. A file renamed in Obsidian keeps its record; only the
//! *type folder* carries meaning.

use chronacle_core::{VaultKey, VaultRecord, VaultScope};

/// Recognised entity type folders. Mirrors the eight per-type tables.
pub const ENTITY_TYPES: [&str; 8] = [
    "npc",
    "location",
    "faction",
    "creature",
    "item",
    "event",
    "player_character",
    "misc",
];

/// Windows reserved device names (case-insensitive, with or without an
/// extension — `con.md` is rejected exactly like `con`). Chronacle's CI
/// builds and ships for Windows (the tauri e2e matrix runs ubuntu, windows,
/// and macos), and a campaign or entity named e.g. "Con" would otherwise
/// produce a `campaigns/con/` directory that Windows refuses to create.
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Lowercase, Unicode-aware, hyphen-separated. Never empty — falls back to
/// `"untitled"` — and never a Windows-reserved device name.
///
/// Any letter or digit of any script survives (`char::is_alphanumeric`);
/// everything else — punctuation, symbols, whitespace, control characters,
/// path separators (`/`, `\`, `:`) — becomes a separator, runs of which
/// collapse to a single `-` with leading/trailing ones trimmed. This is a
/// TTRPG app whose users write fantasy names full of diacritics ("Höhle",
/// "Séraphina", "日本語"); the vault is exactly what the GM looks at in
/// Obsidian, so an ASCII-only slug would mangle most of those names. A
/// `char` can lowercase to more than one `char` (e.g. `İ`), hence pushing
/// the whole `to_lowercase()` iterator rather than a single mapped char.
///
/// macOS normalizes Unicode filenames to NFD where Linux/Windows keep NFC;
/// that is harmless here because reconcile matches records to files via the
/// frontmatter `id` (see [`VaultIndex`]), never via the path — do not
/// "fix" this by normalizing the slug.
///
/// After slugging, a result that collides case-insensitively with a
/// [`RESERVED`] Windows device name gets a `-x` suffix (`con` → `con-x`);
/// this only fires on an exact match, so `Contact` and `company` are left
/// alone. A trailing `.` or space (also rejected by Windows at the end of a
/// path component) is stripped; separator collapsing already prevents a
/// trailing `-`.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true; // suppresses a leading dash
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out = "untitled".to_owned();
    }
    if RESERVED.iter().any(|r| out.eq_ignore_ascii_case(r)) {
        out.push_str("-x");
    }
    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Strips a `table:` prefix from a thing string. A string with no colon is
/// returned unchanged.
fn raw_id(thing: &str) -> &str {
    match thing.split_once(':') {
        Some((_, id)) => id,
        None => thing,
    }
}

/// The folder a scope's records are nested under, e.g. `campaigns/<slug>`.
pub fn scope_folder(scope: &VaultScope) -> String {
    scope_folder_disambiguated(scope, false)
}

/// As [`scope_folder`], but appends `-{raw_id}` when `collides` is set — two
/// campaigns (or collections) may share a display name, since `campaign.name`
/// carries no unique index.
pub fn scope_folder_disambiguated(scope: &VaultScope, collides: bool) -> String {
    let (root, id, name) = match scope {
        VaultScope::Campaign { id, name } => ("campaigns", id, name),
        VaultScope::Collection { id, name } => ("collections", id, name),
    };
    let mut folder = format!("{root}/{}", slug(name));
    if collides {
        folder.push('-');
        folder.push_str(raw_id(id));
    }
    folder
}

/// Builds the vault key for `record`. `collides` appends an `-{id}` suffix to
/// the filename when a caller has determined the un-suffixed key collides
/// with another record's.
pub fn key_for(record: &VaultRecord, collides: bool) -> VaultKey {
    match record {
        VaultRecord::Entity(e) => {
            let suffix = if collides {
                format!("-{}", raw_id(&e.vref.id))
            } else {
                String::new()
            };
            format!(
                "{}/entities/{}/{}{suffix}.md",
                scope_folder(&e.scope),
                e.vref.table,
                slug(&e.name)
            )
        }
        VaultRecord::Session(s) => {
            let suffix = if collides {
                format!("-{}", raw_id(&s.vref.id))
            } else {
                String::new()
            };
            format!(
                "{}/sessions/{:03}-{}{suffix}.md",
                scope_folder(&s.campaign),
                s.session_number,
                slug(&s.title)
            )
        }
        VaultRecord::RuleEntry(r) => {
            let suffix = if collides {
                format!("-{}", raw_id(&r.vref.id))
            } else {
                String::new()
            };
            format!(
                "{}/rules/{}{suffix}.md",
                scope_folder(&r.collection),
                slug(&r.name)
            )
        }
    }
}

/// `true` only for keys matching exactly one of the four managed shapes:
///
/// - `campaigns/<slug>/entities/<type>/<file>.md`
/// - `campaigns/<slug>/sessions/<file>.md`
/// - `collections/<slug>/entities/<type>/<file>.md`
/// - `collections/<slug>/rules/<file>.md`
///
/// `<type>` must be a member of [`ENTITY_TYPES`]; `<slug>` and `<file>` must be
/// non-empty and contain no further `/`; the key must end in a lowercase
/// `.md`; and a `*.conflict.*.md` (or `*.conflict.md`) file is unmanaged.
/// Everything else — including anything outside `campaigns/*/…` and
/// `collections/*/…`, shallower paths, unknown section or type folders, and
/// mismatched root/section pairings (rules are collection-only, sessions are
/// campaign-only) — is unmanaged. This is the gate that keeps a stray vault
/// file from being mistaken for a record to inbound-sync.
pub fn is_managed(key: &str) -> bool {
    if key.starts_with('/') || !key.ends_with(".md") {
        return false;
    }
    let segments: Vec<&str> = key.split('/').collect();
    if segments.iter().any(|s| s.is_empty() || *s == "..") {
        return false;
    }
    let filename = *segments.last().unwrap_or(&"");
    if let Some(stem) = filename.strip_suffix(".md") {
        if stem.contains(".conflict.") || stem.ends_with(".conflict") {
            return false;
        }
    }
    match segments.as_slice() {
        ["campaigns", _slug, "entities", ty, _file] => ENTITY_TYPES.contains(ty),
        ["campaigns", _slug, "sessions", _file] => true,
        ["collections", _slug, "entities", ty, _file] => ENTITY_TYPES.contains(ty),
        ["collections", _slug, "rules", _file] => true,
        _ => false,
    }
}

/// The compiler-owned conflict sidecar for `key`. `is_managed` already treats
/// `*.conflict.md` as unmanaged, so a sidecar can never hijack the index.
pub fn sidecar_key(key: &str) -> VaultKey {
    match key.strip_suffix(".md") {
        Some(stem) => format!("{stem}.conflict.md"),
        None => format!("{key}.conflict.md"),
    }
}

/// Returns the segment after `entities/` in `key`, if it is a recognised
/// entity type.
pub fn entity_type_of(key: &str) -> Option<&str> {
    let segments: Vec<&str> = key.split('/').collect();
    let idx = segments.iter().position(|&s| s == "entities")?;
    let ty = *segments.get(idx + 1)?;
    if ENTITY_TYPES.contains(&ty) {
        Some(ty)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{
        EntityRecord, RuleEntryRecord, SessionRecord, VaultRecord, VaultRef, VaultScope,
    };
    use pretty_assertions::assert_eq;

    fn campaign() -> VaultScope {
        VaultScope::Campaign {
            id: "campaign:c1".into(),
            name: "Shadows of Valdris".into(),
        }
    }
    fn collection() -> VaultScope {
        VaultScope::Collection {
            id: "collection:k1".into(),
            name: "D&D 5e Core".into(),
        }
    }
    fn npc(name: &str, id: &str, scope: VaultScope) -> VaultRecord {
        VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: id.into(),
            },
            name: name.into(),
            summary: None,
            notes: None,
            codex_article: None,
            aliases: vec![],
            scope,
            created_at: "x".into(),
            updated_at: "y".into(),
        })
    }

    #[test]
    fn slug_lowercases_and_hyphenates() {
        assert_eq!(slug("Seraphina Aldric"), "seraphina-aldric");
        assert_eq!(slug("The Iron Tower"), "the-iron-tower");
    }

    #[test]
    fn slug_strips_punctuation_and_collapses_separators() {
        assert_eq!(slug("Vex: The Unbound!"), "vex-the-unbound");
        assert_eq!(slug("A  --  B"), "a-b");
        assert_eq!(slug("  padded  "), "padded");
    }

    #[test]
    fn slug_never_returns_empty() {
        // A name of pure punctuation must still produce a usable filename.
        assert_eq!(slug("???"), "untitled");
        assert_eq!(slug(""), "untitled");
    }

    #[test]
    fn slug_is_unicode_aware() {
        assert_eq!(slug("Höhle des Drachen"), "höhle-des-drachen");
        assert_eq!(slug("Nußbaum"), "nußbaum");
        assert_eq!(slug("Séraphina Aldric"), "séraphina-aldric");
        assert_eq!(slug("日本語"), "日本語");
        assert_eq!(slug("🗡"), "untitled", "emoji alone is not alphanumeric");
    }

    #[test]
    fn slug_still_handles_ascii_cases() {
        assert_eq!(slug("Seraphina Aldric"), "seraphina-aldric");
        assert_eq!(slug("Vex: The Unbound!"), "vex-the-unbound");
        assert_eq!(slug("A  --  B"), "a-b");
        assert_eq!(slug("???"), "untitled");
    }

    #[test]
    fn slug_never_lets_a_path_separator_or_control_char_survive() {
        let s = slug("a/b\\c:d\u{0}e");
        assert!(!s.contains('/'), "got {s}");
        assert!(!s.contains('\\'), "got {s}");
        assert!(!s.contains(':'), "got {s}");
        assert!(!s.chars().any(|c| c.is_control()), "got {s}");
    }

    #[test]
    fn slug_escapes_windows_reserved_device_names() {
        assert_eq!(slug("Con"), "con-x");
        assert_eq!(slug("AUX"), "aux-x");
        assert_eq!(slug("com1"), "com1-x");
        assert_eq!(slug("nul"), "nul-x");
        assert_eq!(slug("LPT9"), "lpt9-x");
        // A name that merely starts with a reserved word is not reserved.
        assert_eq!(slug("Contact"), "contact");
        assert_eq!(slug("company"), "company");
    }

    #[test]
    fn slug_strips_a_trailing_dot() {
        assert_eq!(slug("name."), "name");
    }

    #[test]
    fn scope_folder_roots_campaigns_and_collections_separately() {
        assert_eq!(scope_folder(&campaign()), "campaigns/shadows-of-valdris");
        assert_eq!(scope_folder(&collection()), "collections/d-d-5e-core");
    }

    #[test]
    fn scope_folder_suffixes_on_collision() {
        // Two campaigns may share a name — campaign.name has no UNIQUE index.
        let a = VaultScope::Campaign {
            id: "campaign:aaa".into(),
            name: "Guard Duty".into(),
        };
        let b = VaultScope::Campaign {
            id: "campaign:bbb".into(),
            name: "Guard Duty".into(),
        };
        assert_ne!(
            scope_folder_disambiguated(&a, true),
            scope_folder_disambiguated(&b, true)
        );
        assert!(scope_folder_disambiguated(&a, true).starts_with("campaigns/guard-duty-"));
    }

    #[test]
    fn key_for_entity_nests_under_scope_and_type() {
        let k = key_for(&npc("Seraphina Aldric", "abc123", campaign()), false);
        assert_eq!(
            k,
            "campaigns/shadows-of-valdris/entities/npc/seraphina-aldric.md"
        );
    }

    #[test]
    fn key_for_collection_owned_entity_uses_the_collections_root() {
        let k = key_for(&npc("Goblin", "g1", collection()), false);
        assert_eq!(k, "collections/d-d-5e-core/entities/npc/goblin.md");
    }

    #[test]
    fn key_for_appends_an_id_suffix_on_collision() {
        let a = key_for(&npc("Guard", "4f2a1c", campaign()), true);
        let b = key_for(&npc("Guard", "9e8d7b", campaign()), true);
        assert_ne!(a, b);
        assert!(a.ends_with("/guard-4f2a1c.md"), "got {a}");
    }

    #[test]
    fn key_for_session_uses_a_zero_padded_number() {
        let rec = VaultRecord::Session(SessionRecord {
            vref: VaultRef {
                table: "session".into(),
                id: "s1".into(),
            },
            session_number: 1,
            title: "The Awakening".into(),
            date_played: "2026-01-01".into(),
            notes: String::new(),
            campaign: campaign(),
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        assert_eq!(
            key_for(&rec, false),
            "campaigns/shadows-of-valdris/sessions/001-the-awakening.md"
        );
    }

    #[test]
    fn key_for_rule_entry_lands_under_rules() {
        let rec = VaultRecord::RuleEntry(RuleEntryRecord {
            vref: VaultRef {
                table: "rule_entry".into(),
                id: "r1".into(),
            },
            name: "Grappling".into(),
            category: "procedure".into(),
            body: String::new(),
            notes: None,
            page_refs: vec![],
            aliases: vec![],
            collection: collection(),
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        assert_eq!(
            key_for(&rec, false),
            "collections/d-d-5e-core/rules/grappling.md"
        );
    }

    #[test]
    fn is_managed_accepts_only_the_two_roots() {
        assert!(is_managed("campaigns/x/entities/npc/a.md"));
        assert!(is_managed("collections/x/rules/a.md"));
        assert!(!is_managed("a.md"), "vault root is unmanaged");
        assert!(!is_managed(".obsidian/workspace.json"));
        assert!(!is_managed("campaigns/x/entities/npc/a.conflict.123.md"));
        assert!(!is_managed("Templates/entity.md"));
    }

    #[test]
    fn is_managed_accepts_only_the_four_exact_shapes() {
        // managed
        assert!(is_managed("campaigns/c/entities/npc/a.md"));
        assert!(is_managed("campaigns/c/sessions/001-a.md"));
        assert!(is_managed("collections/k/entities/creature/g.md"));
        assert!(is_managed("collections/k/rules/g.md"));

        // unmanaged
        assert!(!is_managed("a.md"), "vault root");
        assert!(!is_managed("campaigns"), "no file");
        assert!(!is_managed("campaigns/"), "no file, trailing slash");
        assert!(!is_managed("campaigns/c.md"), "too shallow");
        assert!(
            !is_managed("campaigns/x/entities/wizard/a.md"),
            "type not in ENTITY_TYPES"
        );
        assert!(
            !is_managed("campaigns/x/notes/a.md"),
            "unknown section folder"
        );
        assert!(
            !is_managed("campaigns/x/rules/g.md"),
            "rules are collection-scoped"
        );
        assert!(
            !is_managed("collections/k/sessions/1.md"),
            "sessions are campaign-scoped"
        );
        assert!(!is_managed("campaigns/c/entities/npc/a.conflict.123.md"));
        assert!(!is_managed("collections/k/rules/g.conflict.1.md"));
        assert!(!is_managed("campaigns/c/entities/npc/a.txt"));
        assert!(
            !is_managed("campaigns/c/entities/npc/a.MD"),
            "case-sensitive"
        );
        assert!(!is_managed(".obsidian/workspace.json"));
        assert!(!is_managed("Templates/entity.md"));
        assert!(!is_managed("notcampaigns/c/entities/npc/a.md"));
        assert!(!is_managed("campaigns//entities/npc/a.md"), "empty segment");
        assert!(
            !is_managed("campaigns/c/entities/npc/../../../etc/passwd.md"),
            ".. segment"
        );
    }

    #[test]
    fn key_for_always_produces_a_managed_key() {
        let scopes = [campaign(), collection()];
        for scope in scopes {
            let entity = npc("Test", "t1", scope.clone());
            for collides in [false, true] {
                let k = key_for(&entity, collides);
                assert!(is_managed(&k), "entity key {k} should be managed");
            }
        }

        // A Windows-reserved-name entity and a diacritic-heavy entity must
        // both still land on a managed key.
        for name in ["Con", "Höhle des Drachen"] {
            let entity = npc(name, "t2", campaign());
            for collides in [false, true] {
                let k = key_for(&entity, collides);
                assert!(is_managed(&k), "entity key {k} should be managed");
            }
        }

        let session = VaultRecord::Session(SessionRecord {
            vref: VaultRef {
                table: "session".into(),
                id: "s1".into(),
            },
            session_number: 1,
            title: "The Awakening".into(),
            date_played: "2026-01-01".into(),
            notes: String::new(),
            campaign: campaign(),
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        for collides in [false, true] {
            let k = key_for(&session, collides);
            assert!(is_managed(&k), "session key {k} should be managed");
        }

        let rule = VaultRecord::RuleEntry(RuleEntryRecord {
            vref: VaultRef {
                table: "rule_entry".into(),
                id: "r1".into(),
            },
            name: "Grappling".into(),
            category: "procedure".into(),
            body: String::new(),
            notes: None,
            page_refs: vec![],
            aliases: vec![],
            collection: collection(),
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        for collides in [false, true] {
            let k = key_for(&rule, collides);
            assert!(is_managed(&k), "rule key {k} should be managed");
        }
    }

    #[test]
    fn sidecar_key_is_always_unmanaged() {
        let k = sidecar_key("campaigns/c/entities/npc/a.md");
        assert_eq!(k, "campaigns/c/entities/npc/a.conflict.md");
        assert!(!is_managed(&k));
    }

    #[test]
    fn entity_type_of_reads_the_type_folder() {
        assert_eq!(entity_type_of("campaigns/x/entities/npc/a.md"), Some("npc"));
        assert_eq!(entity_type_of("collections/x/rules/a.md"), None);
        assert_eq!(entity_type_of("campaigns/x/sessions/001-a.md"), None);
    }
}

/// `id → key` map built by scanning the vault and reading frontmatter.
///
/// Reconcile matches records to files through this map. It never computes an
/// expected slug: filenames derive from `name`, which is neither unique
/// (two NPCs called "Guard") nor stable (a rename would orphan the file).
#[derive(Debug, Default)]
pub struct VaultIndex {
    by_ref: std::collections::HashMap<chronacle_core::VaultRef, VaultKey>,
    /// Every managed key seen on disk, whether or not its frontmatter parsed.
    /// A record whose file's frontmatter got corrupted (Apply's `invalid`
    /// bucket) still needs its content read — `by_ref` alone can't find it.
    managed_keys: std::collections::HashSet<VaultKey>,
}

impl VaultIndex {
    /// Read every managed `.md` under the vault root and index it by `id`.
    ///
    /// Files with no frontmatter, or unparsable frontmatter, are not mapped
    /// by ref — they are inbound-apply "invalid" candidates, not create
    /// candidates — but they are still recorded in `managed_keys`.
    pub async fn scan(store: &dyn chronacle_core::VaultStore) -> Result<Self, crate::VaultError> {
        let mut by_ref = std::collections::HashMap::new();
        let mut managed_keys = std::collections::HashSet::new();
        for key in store.list("").await? {
            if !is_managed(&key) {
                continue;
            }
            managed_keys.insert(key.clone());
            // An unreadable file must not abort the scan — one bad file would
            // otherwise block every record in the vault from ever syncing. It
            // stays in `managed_keys`, so it still reads as "a file exists
            // here" and can never be mistaken for a GM deletion.
            let content = match store.read(&key).await {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("vault: index scan could not read {key}: {e}");
                    continue;
                }
            };
            let Ok((fm, _)) = crate::frontmatter::parse(&content) else {
                continue;
            };
            let Some(vref) = chronacle_core::VaultRef::parse(&fm.id) else {
                continue;
            };
            by_ref.insert(vref, key);
        }
        Ok(Self {
            by_ref,
            managed_keys,
        })
    }

    /// The key currently holding this record, if any.
    pub fn key_of(&self, vref: &chronacle_core::VaultRef) -> Option<&VaultKey> {
        self.by_ref.get(vref)
    }

    /// Whether the vault holds a file for this record.
    pub fn contains(&self, vref: &chronacle_core::VaultRef) -> bool {
        self.by_ref.contains_key(vref)
    }

    /// Whether a managed file exists at `key`, regardless of whether its
    /// frontmatter parsed. Used to detect a corrupted file at the
    /// record's computed slug when the id-based lookup misses it.
    pub fn has_key(&self, key: &str) -> bool {
        self.managed_keys.contains(key)
    }

    /// Number of indexed records.
    pub fn len(&self) -> usize {
        self.by_ref.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.by_ref.is_empty()
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use chronacle_core::{MockVaultStore, VaultRef};
    use mockall::predicate::eq;

    fn file_with_id(id: &str) -> String {
        format!("---\nid: \"{id}\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nbody\n")
    }

    #[tokio::test]
    async fn scan_maps_ids_to_keys_regardless_of_filename() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| {
            Ok(vec![
                "campaigns/c/entities/npc/renamed-by-the-gm.md".to_string()
            ])
        });
        store
            .expect_read()
            .with(eq("campaigns/c/entities/npc/renamed-by-the-gm.md"))
            .returning(|_| Ok(file_with_id("npc:abc123")));

        let idx = VaultIndex::scan(&store).await.expect("scan");
        let vref = VaultRef {
            table: "npc".into(),
            id: "abc123".into(),
        };
        assert_eq!(
            idx.key_of(&vref).map(String::as_str),
            Some("campaigns/c/entities/npc/renamed-by-the-gm.md"),
            "identity is the frontmatter id, not the slug"
        );
    }

    #[tokio::test]
    async fn scan_ignores_unmanaged_keys() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| {
            Ok(vec![
                "Templates/entity.md".to_string(),
                ".obsidian/workspace.json".to_string(),
                "campaigns/c/entities/npc/a.conflict.9.md".to_string(),
            ])
        });
        // read() must never be called for an unmanaged key.
        store.expect_read().never();

        let idx = VaultIndex::scan(&store).await.expect("scan");
        assert_eq!(idx.len(), 0);
    }

    #[tokio::test]
    async fn scan_skips_a_managed_file_with_no_frontmatter() {
        let mut store = MockVaultStore::new();
        store
            .expect_list()
            .returning(|_| Ok(vec!["campaigns/c/entities/npc/new.md".to_string()]));
        store
            .expect_read()
            .returning(|_| Ok("just prose, no frontmatter\n".to_string()));

        // An id-less file is a tranche-5 create candidate, not an index entry,
        // and must not abort the scan.
        let idx = VaultIndex::scan(&store)
            .await
            .expect("scan must tolerate id-less files");
        assert_eq!(idx.len(), 0);
    }

    #[tokio::test]
    async fn scan_records_the_slug_to_scope_map() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| {
            Ok(vec![
                "campaigns/shadows-of-valdris/entities/npc/a.md".to_string()
            ])
        });
        store
            .expect_read()
            .returning(|_| Ok(file_with_id("npc:a1")));

        let idx = VaultIndex::scan(&store).await.expect("scan");
        assert!(idx.contains(&VaultRef {
            table: "npc".into(),
            id: "a1".into()
        }));
    }
}
