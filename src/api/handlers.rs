use crate::interfaces::SetRequestData;
use futures::lock::Mutex;
use std::sync::Arc;
use valence_core::api::errors::ApiErrorType;
use valence_core::api::interfaces::CFilterConnection;
use valence_core::api::responses::{json_serialize_embed, CallResponse, JsonReply};
use valence_core::db::handler::{CacheHandler, KvStoreConnection};
use valence_core::utils::{deserialize_data, serialize_data};

// ========= BASE HANDLERS ========= //

/// Route to get data from DB
///
/// ### Arguments
///
/// * `payload` - Request payload
/// * `db` - Database connection
/// * `cache` - Cache connection
/// * `c_filter` - Cuckoo filter connection
pub async fn get_data_handler<
    D: KvStoreConnection + Clone + Send + 'static,
    C: KvStoreConnection + Clone + Send + 'static,
>(
    headers: warp::hyper::HeaderMap,
    db: Arc<Mutex<D>>,
    cache: Arc<Mutex<C>>,
    c_filter: CFilterConnection,
) -> Result<JsonReply, JsonReply> {
    let r = CallResponse::new("get_data");
    let address = headers
        .get("address")
        .and_then(|n| n.to_str().ok())
        .unwrap_or_default();

    // Check if address is in cuckoo filter
    if !c_filter.lock().await.contains(&address) {
        return r.into_err_internal(ApiErrorType::CuckooFilterLookupFailed);
    }

    // Check cache first
    let cache_result = cache.lock().await.get_data(&address).await;

    match cache_result {
        Ok(value) => {
            // Return data from cache
            let final_data = if value.is_some() {
                deserialize_data::<String>(value.unwrap())
            } else {
                "".to_string()
            };
            r.into_ok(
                "Data retrieved successfully",
                json_serialize_embed(final_data),
            )
        }
        Err(_) => {
            // Get data from DB
            let db_result = db.lock().await.get_data(&address).await;

            match db_result {
                Ok(value) => {
                    // Return data from DB
                    let final_data = deserialize_data::<String>(value.unwrap());
                    r.into_ok(
                        "Data retrieved successfully",
                        json_serialize_embed(final_data),
                    )
                }
                Err(_) => r.into_err_internal(ApiErrorType::DBInsertionFailed),
            }
        }
    }
}

/// Route to set data
///
/// ### Arguments
///
/// * `payload` - Request payload
/// * `db` - Database connection
/// * `cache` - Cache connection
/// * `c_filter` - Cuckoo filter connection
pub async fn set_data_handler<
    D: KvStoreConnection + Clone + Send + 'static,
    C: KvStoreConnection + CacheHandler + Clone + Send + 'static,
>(
    payload: SetRequestData,
    db: Arc<Mutex<D>>,
    cache: Arc<Mutex<C>>,
    c_filter: CFilterConnection,
    cache_ttl: usize,
) -> Result<JsonReply, JsonReply> {
    let r = CallResponse::new("set_data");

    // Add to cache
    let cache_result = cache
        .lock()
        .await
        .set_data(&payload.address.clone(), serialize_data(&payload.data))
        .await;

    // Add to DB
    let db_result = match cache_result {
        Ok(_) => {
            // Set key expiry
            let _ = cache
                .lock()
                .await
                .expire_entry(&payload.address, cache_ttl)
                .await;

            // Set data in DB
            let data = match serde_json::from_str::<serde_json::Value>(&payload.data) {
                Ok(data) => data,
                Err(_) => {
                    return r.into_err_internal(ApiErrorType::DataSerializationFailed);
                }
            };

            db.lock().await.set_data(&payload.address, data).await
        }
        Err(_) => {
            return r.into_err_internal(ApiErrorType::CacheInsertionFailed);
        }
    };

    // Add to cuckoo filter
    let c_filter_result = match db_result {
        Ok(_) => c_filter.lock().await.add(&payload.address),
        Err(_) => {
            return r.into_err_internal(ApiErrorType::DBInsertionFailed);
        }
    };

    match c_filter_result {
        Ok(_) => r.into_ok(
            "Data set succcessfully",
            json_serialize_embed(payload.address),
        ),
        Err(_) => r.into_err_internal(ApiErrorType::CuckooFilterInsertionFailed),
    }
}
