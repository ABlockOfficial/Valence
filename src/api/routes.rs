use crate::api::handlers::{get_data_handler, set_data_handler};
use futures::lock::Mutex;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};
use weaver_core::api::interfaces::CFilterConnection;
use weaver_core::api::utils::{
    get_cors, map_api_res, post_cors, sig_verify_middleware, with_node_component,
};
use weaver_core::db::handler::KvStoreConnection;

// ========== BASE ROUTES ========== //

/// GET /get_data
///
/// Retrieves data associated with a given address
///
/// ### Arguments
///
/// * `db` - The database connection to use
/// * `cache` - The cache connection to use
/// * `cuckoo_filter` - The cuckoo filter connection to use
pub fn get_data<
    D: KvStoreConnection + Clone + Send + 'static,
    C: KvStoreConnection + Clone + Send + 'static,
>(
    db: Arc<Mutex<D>>,
    cache: Arc<Mutex<C>>,
    cuckoo_filter: CFilterConnection,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path("get_data")
        .and(warp::get())
        .and(sig_verify_middleware())
        .and(warp::header::headers_cloned())
        .and(with_node_component(cache))
        .and(with_node_component(db))
        .and(with_node_component(cuckoo_filter))
        .and_then(move |_, headers, cache, db, cf| {
            map_api_res(get_data_handler(headers, db, cache, cf))
        })
        .with(get_cors())
}

/// POST /set_data
///
/// Sets data for a given address
///
/// ### Arguments
///
/// * `db` - The database connection to use
/// * `cache` - The cache connection to use
/// * `cuckoo_filter` - The cuckoo filter connection to use
/// * `body_limit` - The maximum size of the request body
pub fn set_data<
    D: KvStoreConnection + Clone + Send + Sync + 'static,
    C: KvStoreConnection + Clone + Send + Sync + 'static,
>(
    db: Arc<Mutex<D>>,
    cache: Arc<Mutex<C>>,
    cuckoo_filter: CFilterConnection,
    body_limit: u64,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path("set_data")
        .and(warp::post())
        .and(sig_verify_middleware())
        .and(warp::body::content_length_limit(body_limit))
        .and(warp::body::json())
        .and(with_node_component(cache))
        .and(with_node_component(db))
        .and(with_node_component(cuckoo_filter))
        .and_then(move |_, info, cache, db, cf| map_api_res(set_data_handler(info, db, cache, cf)))
        .with(post_cors())
}
