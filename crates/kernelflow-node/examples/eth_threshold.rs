//! Mirrors the Fermah Kernel blog post's canonical example:
//! "median of 3 prices crosses $3000" — composed entirely from verified
//! primitives in `kernelflow-nodes`, validated by the registry, and run by
//! the actor scheduler.
//!
//! Run with: `cargo run -p kernelflow-node --example eth_threshold`

use std::collections::HashMap;
use std::sync::Arc;

use kernelflow_core::{DagBuilder, EdgeCondition, WorkflowNode};
use kernelflow_nodes::{ConstantNode, MedianNode, NodeRegistry, ThresholdNode};
use kernelflow_scheduler::{Scheduler, SchedulerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let registry = NodeRegistry::with_stdlib();
    // Composition-time validation of the wiring (Kernel-style).
    registry.validate_edge("constant", "median")?;
    registry.validate_edge("median",   "threshold")?;

    // Build the DAG: 3 price sources -> median -> threshold>3000.
    let dag = DagBuilder::<&'static str>::new("eth-threshold-3000")
        .node("p1",        "constant")
        .node("p2",        "constant")
        .node("p3",        "constant")
        .node("median",    "median")
        .node("threshold", "threshold")
        .edge("p1", "median", EdgeCondition::Always)
        .edge("p2", "median", EdgeCondition::Always)
        .edge("p3", "median", EdgeCondition::Always)
        .edge("median", "threshold", EdgeCondition::Always)
        .build()?;

    // Hardcoded "prices" for the example. In production these would be
    // `http_fetch` nodes against allowlisted endpoints.
    let prices: HashMap<&'static str, serde_json::Value> = HashMap::from([
        ("p1", serde_json::json!(2980.0)),
        ("p2", serde_json::json!(3050.5)),
        ("p3", serde_json::json!(3100.0)),
    ]);

    let scheduler = Scheduler::new(SchedulerConfig::default());
    let attestation = scheduler.run(&dag, |id, kind| -> Arc<dyn WorkflowNode> {
        match *kind {
            "constant" => {
                let v = prices.get(id).cloned().unwrap_or(serde_json::json!(0.0));
                Arc::new(ConstantNode::new(v))
            }
            "median"    => Arc::new(MedianNode),
            "threshold" => Arc::new(ThresholdNode::gt(3000.0)),
            other       => panic!("unknown kind {other}"),
        }
    }).await?;

    println!("attestation hash: {attestation}");
    Ok(())
}

