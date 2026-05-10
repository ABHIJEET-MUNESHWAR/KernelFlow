use kernelflow_core::{DagBuilder, EdgeCondition};

#[test]
fn end_to_end_dag_topology() {
    // Build a small workflow: ingest -> validate -> [attest, notify].
    let dag = DagBuilder::<&'static str>::new("e2e")
        .node("ingest",   "wasm:ingest")
        .node("validate", "wasm:validate")
        .node("attest",   "native:attest")
        .node("notify",   "native:notify")
        .edge("ingest",   "validate", EdgeCondition::Always)
        .edge("validate", "attest",
              EdgeCondition::JsonEq { pointer: "/ok".into(), value: serde_json::json!(true) })
        .edge("validate", "notify",   EdgeCondition::Always)
        .build()
        .expect("valid DAG");

    let order = dag.topo_sort().unwrap();
    assert_eq!(order[0], "ingest");
    assert_eq!(order[1], "validate");
    // attest & notify can be in either order.
    assert!(order[2..].contains(&"attest".to_string()));
    assert!(order[2..].contains(&"notify".to_string()));
}

