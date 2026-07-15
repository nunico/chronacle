//! Entity graph service — the eight node tables, their scope edges, and the
//! `relates_to` relationship edges between them.
//!
//! Split by concern:
//! - [`types`] — `EntityError`, `EntityKind`, `RelType`, and the record/DTO structs
//! - [`crud`] — create / read / update / delete plus timeline + embedding helpers
//! - [`relations`] — `relates_to` edges, the ego graph, and the flat relations list
//! - [`wikilink_backfill`] — one-shot resync of `[[wikilinks]]` across all entities
//! - [`aliases`] — alternate-name management (tier-4 fuzzy auto-resolve persistence)

mod aliases;
mod crud;
mod relations;
mod types;
mod wikilink_backfill;

pub use aliases::*;
pub use crud::*;
pub use relations::*;
pub use types::*;
pub use wikilink_backfill::*;

/// Appended to every SELECT that needs to populate `campaign` and `collection`
/// in `GraphNodeRecord` via backward edge traversal.
///
/// Using `array::first(...)` to project a single record (or NULL when no edge
/// exists) from the `in_campaign` / `in_collection` edge tables.
const SELECT_SCOPE_ALIASES: &str = "array::first(<-in_campaign<-campaign) AS campaign, \
     array::first(<-in_collection<-collection) AS collection";
