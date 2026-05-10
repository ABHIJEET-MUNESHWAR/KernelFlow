//! # kernelflow-ai
//!
//! Agentic AI surface area:
//! * [`WorkflowSynthesizer`] — given a natural-language goal, an LLM emits a
//!   `Dag<NodeSpec>` (validated by `kernelflow-core`'s topological check
//!   before execution).
//! * [`AnomalyAgent`] — subscribes to `KernelEvent`s; when failure rate spikes
//!   it asks an LLM to summarize root cause and propose a remediation patch.
//!
//! The provider is abstracted behind [`LlmProvider`] (DIP) so OpenAI,
//! local Ollama, or a mock can be swapped without touching call sites.

use async_trait::async_trait;
use kernelflow_core::KernelResult;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> KernelResult<String>;
}

/// OpenAI-compatible HTTP provider (works with OpenAI, Ollama OpenAI shim, vLLM).
pub struct OpenAiCompat {
    pub base_url: String,
    pub api_key:  String,
    pub model:    String,
}

#[async_trait]
impl LlmProvider for OpenAiCompat {
    async fn complete(&self, system: &str, user: &str) -> KernelResult<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role":"system","content": system},
                {"role":"user",  "content": user},
            ]
        });
        let res = reqwest::Client::new()
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key).json(&body).send().await
            .map_err(|e| kernelflow_core::KernelError::Network(e.to_string()))?
            .json::<serde_json::Value>().await
            .map_err(|e| kernelflow_core::KernelError::Network(e.to_string()))?;
        Ok(res["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec { pub id: String, pub kind: String, pub args: serde_json::Value }

pub struct WorkflowSynthesizer<P: LlmProvider> { pub llm: P }

impl<P: LlmProvider> WorkflowSynthesizer<P> {
    pub async fn synthesize(&self, goal: &str) -> KernelResult<Vec<NodeSpec>> {
        let sys  = "You output strictly JSON: an array of {id,kind,args} workflow nodes.";
        let raw  = self.llm.complete(sys, goal).await?;
        serde_json::from_str(&raw).map_err(|e| kernelflow_core::KernelError::Serde(e))
    }
}

/// Mock provider for tests.
pub struct MockLlm(pub String);
#[async_trait]
impl LlmProvider for MockLlm {
    async fn complete(&self, _: &str, _: &str) -> KernelResult<String> { Ok(self.0.clone()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn synth_with_mock() {
        let llm = MockLlm(r#"[{"id":"a","kind":"wasm","args":{}}]"#.into());
        let s = WorkflowSynthesizer { llm };
        let nodes = s.synthesize("do x").await.unwrap();
        assert_eq!(nodes.len(), 1);
    }
}

