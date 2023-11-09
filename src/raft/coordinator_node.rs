use std::net::SocketAddr;
use axum_otel_metrics::HttpMetricsLayerBuilder;
use axum_tracing_opentelemetry::middleware::OtelAxumLayer;
use tracing::info;

use super::{memstore::MemNodeId, coordinator_config::CoordinatorRaftApp};
use std::sync::Arc;
use axum::{Extension, Router};
use crate::{raft::{raft_api, management}, coordinator::shutdown_signal, server_config::CoordinatorConfig};


pub struct RaftCoordinatorNode {
    addr: SocketAddr,
    coordinator: Arc<CoordinatorRaftApp>,
}

impl RaftCoordinatorNode {
    pub async fn new(id: MemNodeId, addr: String, raft_config: openraft::Config, coordinator_config: CoordinatorConfig) -> Result<Self, anyhow::Error> {
        let coordinator = CoordinatorRaftApp::new(id, addr, raft_config, coordinator_config).await?;
        let addr: SocketAddr = coordinator.addr.parse()?;
        info!("coordinator listening on: {}", addr.to_string());
        Ok(Self { addr, coordinator: Arc::new(coordinator) })
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let metrics = HttpMetricsLayerBuilder::new().build();
        let app = Router::new()
            .merge(metrics.routes())
            .route("/", axum::routing::get(root))

            // raft internal RPC
			.route("/raft/append", axum::routing::post(raft_api::append))
			.route("/raft/snapshot", axum::routing::post(raft_api::snapshot))
			.route("/raft/vote", axum::routing::post(raft_api::vote))

			// admin API
			.route("/management/init", axum::routing::post(management::init))
			.route("/management/add_learner", axum::routing::post(management::add_learner))
			.route("/management/change_membership", axum::routing::post(management::change_membership))
			.route("/management/metrics", axum::routing::get(management::metrics))

			// application API
			.route("/sync_executor", axum::routing::post(super::api::sync_executor))
			.route("/executors", axum::routing::get(super::api::list_executors))
            .route("/create_work", axum::routing::post(super::api::create_work))
			.route("/embed_query", axum::routing::post(super::api::embed_query))

			.layer(Extension(self.coordinator.clone()))
			//start OpenTelemetry trace on incoming request
			.layer(OtelAxumLayer::default())
            .layer(metrics);

        // run the server
        axum::Server::bind(&self.addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }

    pub async fn run_extractors(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

async fn root() -> &'static str {
    "Indexify Coordinator Raft Node"
}
