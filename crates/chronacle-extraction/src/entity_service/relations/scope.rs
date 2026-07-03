//! Reference-scope validation for `relates_to` edges (ADR-009).
//!
//! Rules (symmetric on the unordered pair — ADR-010 already treats
//! owned↔subscribed cross-edges as legitimate in either direction):
//! * same collection → allowed;
//! * a pair {campaign-governed content, collection X} → allowed iff that
//!   campaign subscribes to X;
//! * two different regular collections → violation;
//! * an endpoint with no scope edges at all (legacy/test data) → allowed —
//!   we cannot judge it, and blocking would break pre-scope data.

use serde::Deserialize;
use surrealdb::sql::Thing;

use super::super::EntityError;

#[derive(Debug, Deserialize)]
struct EndpointScope {
    collection: Option<Thing>,
    campaign: Option<Thing>,
}

impl EndpointScope {
    fn unscoped(&self) -> bool {
        self.collection.is_none() && self.campaign.is_none()
    }
}

/// Resolve where an entity lives: its collection (`in_collection` edge) and
/// the campaign that governs it (`in_campaign` edge, or the collection's
/// `owner_campaign` when the collection is campaign-bound).
async fn endpoint_scope<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    id: &str,
) -> Result<EndpointScope, EntityError> {
    // kind/id are validated by the callers in `edge.rs` (is_safe_record_id).
    let q = format!(
        "LET $col = array::first((SELECT VALUE in FROM in_collection WHERE out = {kind}:{id}));
         LET $cam = array::first((SELECT VALUE in FROM in_campaign WHERE out = {kind}:{id}));
         LET $owner = IF $col IS NOT NONE THEN $col.owner_campaign ELSE NONE END;
         RETURN {{ collection: $col, campaign: $cam ?? $owner }};"
    );
    let mut resp = db.query(q).await.map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    // Statements 0-2 are LETs; the RETURN is index 3.
    let scope: Option<EndpointScope> = resp.take(3).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(scope.unwrap_or(EndpointScope {
        collection: None,
        campaign: None,
    }))
}

async fn is_subscribed<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign: &Thing,
    collection: &Thing,
) -> Result<bool, EntityError> {
    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }
    let mut resp = db
        .query("SELECT count() FROM subscribes_to WHERE in = $cam AND out = $col GROUP ALL")
        .bind(("cam", campaign.clone()))
        .bind(("col", collection.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let rows: Vec<CountRow> = resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(rows.first().map(|r| r.count).unwrap_or(0) > 0)
}

/// Enforce the reference rules for a prospective `relates_to` edge.
pub(super) async fn check_scope<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    from_kind: &str,
    from_id: &str,
    to_kind: &str,
    to_id: &str,
) -> Result<(), EntityError> {
    let a = endpoint_scope(db, from_kind, from_id).await?;
    let b = endpoint_scope(db, to_kind, to_id).await?;

    if a.unscoped() || b.unscoped() {
        return Ok(());
    }
    if a.collection.is_some() && a.collection == b.collection {
        return Ok(());
    }
    if a.campaign.is_some() && a.campaign == b.campaign {
        return Ok(());
    }
    // One side campaign-governed, other side in a collection that campaign
    // subscribes to (checked both ways — the rule is pair-symmetric).
    for (gov, other) in [(&a, &b), (&b, &a)] {
        if let (Some(cam), Some(col)) = (&gov.campaign, &other.collection) {
            if other.campaign.is_none() && is_subscribed(db, cam, col).await? {
                return Ok(());
            }
        }
    }

    Err(EntityError::ScopeViolation {
        from: format!("{from_kind}:{from_id}"),
        to: format!("{to_kind}:{to_id}"),
    })
}
