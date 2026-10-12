//! Owner-only local broker request dispatch. Transport authorization happens before dispatch.

use hh_core::error::Error;
use hh_core::local_ipc::{
    AlbumsCreateReq, AlbumsDeleteReq, AlbumsItemsAddReq, AlbumsItemsListQuery,
    AlbumsItemsRemoveReq, AlbumsListQuery, AlbumsUpdateReq, LocalIpcError, LocalIpcRequest,
    LocalIpcResponse, LocalOp,
};
use hh_db::Db;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

fn payload<T: DeserializeOwned>(value: Value) -> Result<T, LocalIpcError> {
    serde_json::from_value(value).map_err(|e| LocalIpcError::InvalidInput(e.to_string()))
}

fn db_error(error: Error) -> LocalIpcError {
    match error {
        Error::BadRequest(message) | Error::TooLarge(message) => {
            LocalIpcError::InvalidInput(message)
        }
        Error::NotFound(message) => LocalIpcError::NotFound(message),
        Error::Conflict(message) => LocalIpcError::Conflict(message),
        Error::ForbiddenScope(message) => LocalIpcError::Forbidden(message),
        other => LocalIpcError::Internal(other.to_string()),
    }
}

pub fn dispatch_local_request(db: &Db, req: LocalIpcRequest) -> LocalIpcResponse {
    let result: Result<Value, LocalIpcError> = (|| match req.op {
        LocalOp::AlbumsList => {
            let query: AlbumsListQuery = payload(req.payload)?;
            let (items, next_cursor) = db
                .albums_list(query.cursor.as_deref(), query.limit)
                .map_err(db_error)?;
            Ok(json!({ "items": items, "next_cursor": next_cursor }))
        }
        LocalOp::AlbumsCreate => {
            let input: AlbumsCreateReq = payload(req.payload)?;
            Ok(json!(db
                .albums_create(&input.id, &input.name)
                .map_err(db_error)?))
        }
        LocalOp::AlbumsUpdate => {
            let input: AlbumsUpdateReq = payload(req.payload)?;
            Ok(json!(db
                .albums_update(&input.id, &input.expected_name, &input.name)
                .map_err(db_error)?))
        }
        LocalOp::AlbumsDelete => {
            let input: AlbumsDeleteReq = payload(req.payload)?;
            Ok(json!({ "deleted": db.albums_delete(&input.id).map_err(db_error)? }))
        }
        LocalOp::AlbumsItemsList => {
            let query: AlbumsItemsListQuery = payload(req.payload)?;
            let (items, next_cursor) = db
                .albums_items_list(&query.id, query.cursor.as_deref(), query.limit)
                .map_err(db_error)?;
            Ok(json!({ "items": items, "next_cursor": next_cursor }))
        }
        LocalOp::AlbumsItemsAdd => {
            let input: AlbumsItemsAddReq = payload(req.payload)?;
            input.validate()?;
            let (added, already_present) = db
                .albums_items_add(&input.id, &input.file_ids)
                .map_err(db_error)?;
            Ok(json!({ "added": added, "already_present": already_present }))
        }
        LocalOp::AlbumsItemsRemove => {
            let input: AlbumsItemsRemoveReq = payload(req.payload)?;
            input.validate()?;
            let removed = db
                .albums_items_remove(&input.id, &input.file_ids)
                .map_err(db_error)?;
            Ok(json!({ "removed": removed }))
        }
        LocalOp::BackupSummary => Ok(json!(db.backup_summary().map_err(db_error)?)),
        _ => Err(LocalIpcError::Forbidden(
            "unsupported operation on this broker".into(),
        )),
    })();
    match result {
        Ok(data) => LocalIpcResponse {
            id: req.id,
            ok: true,
            data: Some(data),
            error: None,
        },
        Err(error) => LocalIpcResponse {
            id: req.id,
            ok: false,
            data: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_and_backup_dispatch_roundtrip() {
        let db = Db::open_memory().unwrap();
        let id = ulid::Ulid::new().to_string();
        let send = |op, payload| {
            dispatch_local_request(
                &db,
                LocalIpcRequest {
                    id: "request".into(),
                    op,
                    payload,
                },
            )
        };
        let created = send(
            LocalOp::AlbumsCreate,
            json!({ "id": id, "name": "  Holiday  " }),
        );
        assert!(created.ok);
        assert_eq!(created.data.unwrap()["name"], "Holiday");
        let listed = send(
            LocalOp::AlbumsList,
            json!({ "cursor": null, "limit": null }),
        );
        assert_eq!(listed.data.unwrap()["items"][0]["id"], id);
        let renamed = send(
            LocalOp::AlbumsUpdate,
            json!({ "id": id, "expected_name": "Holiday", "name": "Trip" }),
        );
        assert_eq!(renamed.data.unwrap()["name"], "Trip");
        let summary = send(LocalOp::BackupSummary, json!({}));
        assert_eq!(summary.data.unwrap()["total_items"], 0);
        let deleted = send(LocalOp::AlbumsDelete, json!({ "id": id }));
        assert_eq!(deleted.data.unwrap()["deleted"], true);
    }

    #[test]
    fn dispatch_errors_are_typed() {
        let db = Db::open_memory().unwrap();
        let send = |op, payload| {
            dispatch_local_request(
                &db,
                LocalIpcRequest {
                    id: "request".into(),
                    op,
                    payload,
                },
            )
        };
        assert!(matches!(
            send(LocalOp::AppLaunch, json!({})).error,
            Some(LocalIpcError::Forbidden(_))
        ));
        assert!(matches!(
            send(LocalOp::AlbumsCreate, json!({ "id": "bad", "name": "x" })).error,
            Some(LocalIpcError::InvalidInput(_))
        ));
        let id = ulid::Ulid::new().to_string();
        assert!(matches!(
            send(
                LocalOp::AlbumsItemsList,
                json!({ "id": id, "cursor": null, "limit": null })
            )
            .error,
            Some(LocalIpcError::NotFound(_))
        ));
    }
}
