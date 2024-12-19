use anyhow::Result;
use iroh_blobs::format::collection::Collection;
use iroh_blobs::rpc::client::blobs::AddOutcome;
use iroh_blobs::ticket::BlobTicket;

use super::capabilities::CapSet;
use super::users::User;
use super::Space;

// filename for the event data that describes the space when it's in an iroh collection
pub(crate) const SPACE_COLLECTION_FILENAME: &str = "space.json";
pub(crate) const SPACE_COLLECTION_DB_FILENAME: &str = "space.db";

pub async fn export_space(space: &Space, user: &User) -> Result<BlobTicket> {
    let caps = space.capabilities().caps_for_user(user).await?;
    export_space_with_capabilities(space, caps).await
}

// lol what a bunch of hot garbage
// TODO: this doesn't transfer program blobs
pub async fn export_space_with_capabilities(
    space: &Space,
    capabilities: CapSet,
) -> Result<BlobTicket> {
    let blobs = space.router().blobs();

    // use the latest space details as the initial hash
    let info = space.info().await?;
    let space_data = serde_json::to_vec(&info)?;
    let res = blobs.add_bytes(space_data).await?;
    let add_db_result = events_for_cap_set(space, capabilities).await?;

    let collection: Collection = vec![
        (SPACE_COLLECTION_FILENAME, res.hash),
        (SPACE_COLLECTION_DB_FILENAME, add_db_result.hash),
    ]
    .into_iter()
    .collect();

    let (collection_hash, _) = blobs
        .create_collection(
            collection,
            iroh_blobs::util::SetTagOption::Auto,
            vec![add_db_result.tag],
        )
        .await?;

    let addr = space.router().endpoint().node_addr().await?;
    let blob_ticket = BlobTicket::new(addr, collection_hash, iroh_blobs::BlobFormat::HashSeq)?;

    Ok(blob_ticket)
}

/// create an sqlite database of events for a user based on the capabilities they have,
/// add it to iroh blobs, and return the hash
/// TODO(b5) - currently the capabilities are ignored and the entire database is sent
async fn events_for_cap_set(space: &Space, _caps: CapSet) -> Result<AddOutcome> {
    // fuck it, send the entire database
    let db_path = space.path.join(space.db_filename());
    space
        .router()
        .blobs()
        .add_from_path(
            db_path,
            true,
            iroh_blobs::util::SetTagOption::Auto,
            iroh_blobs::rpc::client::blobs::WrapOption::NoWrap,
        )
        .await?
        .finish()
        .await
}
