//! # kernelflow-api
//!
//! GraphQL HTTP + WebSocket API. Three top-level objects:
//!   * `Query.workflow(id)`        — fetch state
//!   * `Mutation.runWorkflow(...)` — submit a DAG
//!   * `Subscription.events`       — live `KernelEvent` stream
//!
//! Why GraphQL over REST: clients fetch only what they need (workflows can
//! have hundreds of nodes), and subscriptions give us first-class reactive
//! event delivery with no extra SSE plumbing.

use std::sync::Arc;

use async_graphql::{Context, EmptyMutation, Object, Schema, SimpleObject, Subscription};
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::{routing::get, Router};
use futures::Stream;
use tokio::sync::broadcast;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use kernelflow_core::KernelEvent;

#[derive(Clone)]
pub struct AppState {
    pub events: broadcast::Sender<KernelEvent>,
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Health probe used by Docker/K8s.
    async fn health(&self) -> &'static str {
        "ok"
    }

    async fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

#[derive(SimpleObject, Clone)]
pub struct EventDto {
    pub kind: String,
    pub payload: String,
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn events<'ctx>(&self, ctx: &Context<'ctx>) -> impl Stream<Item = EventDto> + 'ctx {
        let state = ctx.data_unchecked::<Arc<AppState>>().clone();
        let mut rx = state.events.subscribe();
        async_stream::stream! {
            while let Ok(ev) = rx.recv().await {
                let payload = serde_json::to_string(&ev).unwrap_or_default();
                yield EventDto { kind: format!("{:?}", std::mem::discriminant(&ev)), payload };
            }
        }
    }
}

pub type AppSchema = Schema<QueryRoot, EmptyMutation, SubscriptionRoot>;

pub fn build_router(state: Arc<AppState>) -> Router {
    let schema = Schema::build(QueryRoot, EmptyMutation, SubscriptionRoot)
        .data(state.clone())
        .finish();

    Router::new()
        .route(
            "/graphql",
            get(|| async {
                axum::response::Html(
                    async_graphql::http::GraphiQLSource::build()
                        .endpoint("/graphql")
                        .subscription_endpoint("/ws")
                        .finish(),
                )
            })
            .post_service(GraphQL::new(schema.clone())),
        )
        .route_service("/ws", GraphQLSubscription::new(schema))
        .route("/health", get(|| async { "ok" }))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
