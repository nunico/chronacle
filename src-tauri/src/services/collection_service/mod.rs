/// Collection service — CRUD operations and campaign subscription management.
///
/// A `collection` groups related source PDFs (e.g. "D&D 5e Core Rules").
/// Campaigns subscribe to collections via the `subscribes_to` relation so that
/// multiple campaigns can share the same rulebook set without duplication.
mod crud;
mod subscriptions;
mod types;

pub use crud::{create, delete, get_all, get_by_id, update};
pub use subscriptions::{
    add_campaign_collection, get_campaign_collections, remove_campaign_collection,
};
pub use types::{Collection, CollectionRecord};

#[cfg(test)]
mod tests;
