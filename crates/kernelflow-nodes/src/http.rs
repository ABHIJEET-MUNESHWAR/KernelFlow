//! HTTP fetch node — *capability-scoped* egress with timeout & host allowlist.
//! This is one of the verified primitives the Fermah blog calls out:
//! "an HTTP fetch ... a network of nodes ... bounded by a timeout."

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kernelflow_core::{
    Capability, KernelError, KernelResult, NodeContext, NodeInput, NodeOutput,
    ResourceRequirements, WorkflowNode,
};
use serde::Deserialize;

use crate::{NodeDescriptor, NodeRegistry, TypeSig};

#[derive(Debug, Deserialize)]
struct Args {
    url: String,
    allowlist: Option<Vec<String>>,
}

pub struct HttpFetchNode {
    url: String,
    allowlist: Vec<String>,
    timeout: Duration,
}

impl HttpFetchNode {
    pub fn new(url: impl Into<String>, allowlist: Vec<String>) -> Self {
        Self {
            url: url.into(),
            allowlist,
            timeout: Duration::from_secs(10),
        }
    }
    fn allowed(&self, url: &str) -> bool {
        if self.allowlist.is_empty() {
            return true;
        }
        self.allowlist.iter().any(|host| url.contains(host))
    }
}

#[async_trait]
impl WorkflowNode for HttpFetchNode {
    fn kind(&self) -> &'static str {
        "http_fetch"
    }
    async fn execute(&self, _ctx: &NodeContext, _input: NodeInput) -> KernelResult<NodeOutput> {
        if !self.allowed(&self.url) {
            return Err(KernelError::InvalidInput(format!(
                "host not in allowlist: {}",
                self.url
            )));
        }
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| KernelError::Network(e.to_string()))?;
        let body = client
            .get(&self.url)
            .send()
            .await
            .map_err(|e| KernelError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| KernelError::Network(e.to_string()))?;
        // Try JSON, fall back to string.
        let value: serde_json::Value =
            serde_json::from_str(&body).unwrap_or(serde_json::Value::String(body));
        Ok(NodeOutput {
            value,
            gas_used: 50,
        })
    }
}

pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(
        NodeDescriptor {
            kind: "http_fetch",
            doc: "Fetch a URL via the controlled egress path. Args: { url, allowlist? }",
            input: TypeSig::Json,
            output: TypeSig::Json,
            requirements: ResourceRequirements {
                capabilities: vec![Capability::HttpEgress],
                ..Default::default()
            },
        },
        Arc::new(|args| {
            let a: Args = serde_json::from_value(args.clone())
                .map_err(|e| KernelError::InvalidInput(e.to_string()))?;
            Ok(
                Arc::new(HttpFetchNode::new(a.url, a.allowlist.unwrap_or_default()))
                    as Arc<dyn WorkflowNode>,
            )
        }),
    );
}
