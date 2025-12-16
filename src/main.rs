mod db;
mod email;
mod epub_gen;
mod epub_message;
mod feed;
#[cfg(feature = "mem_opt")]
mod image;
#[cfg(not(feature = "mem_opt"))]
#[path = "image_inmem.rs"]
mod image;
mod models;
mod opds;
mod processor;
mod scheduler;
mod util;
mod handlers;
mod routes;

use crate::models::AppState;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tracing::info;
use db::schema_init;

#[cfg(feature = "alternative-alloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    #[cfg(feature = "mem_opt")]
    let _vips_app = libvips::VipsApp::new("rsspub", false).expect("Failed to initialize libvips");
    #[cfg(feature = "alternative-alloc")]
    tikv_jemalloc_ctl::background_thread::write(true).expect("failed to enable background threads");

    tracing_subscriber::fmt().init();

    let conn = schema_init::init_db("./db/rpub.db").expect("Failed to initialize database");
    let db_mutex = Arc::new(Mutex::new(conn));
    let sched = scheduler::init_scheduler(db_mutex.clone())
        .await
        .expect("Failed to initialize scheduler");

    let state = Arc::new(AppState {
        db: db_mutex.clone(),
        scheduler: Arc::new(TokioMutex::new(sched)),
    });

    tokio::fs::create_dir_all(util::EPUB_OUTPUT_DIR).await.unwrap();

    let app = routes::create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
