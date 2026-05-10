//! Per-node actor. One spawned tokio task that owns its `WorkflowNode` impl
//! and processes one input at a time from its mailbox.

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use kernelflow_core::{KernelResult, NodeContext, NodeInput, NodeOutput, WorkflowNode};

use crate::resilience::ResilientExecutor;

pub struct NodeActor {
    tx: mpsc::Sender<Msg>,
}

struct Msg {
    input: NodeInput,
    ctx: NodeContext,
    reply: oneshot::Sender<KernelResult<NodeOutput>>,
}

impl NodeActor {
    pub fn spawn(node: Arc<dyn WorkflowNode>, exec: Arc<ResilientExecutor>) -> Self {
        let (tx, mut rx) = mpsc::channel::<Msg>(64);
        tokio::spawn(async move {
            while let Some(Msg { input, ctx, reply }) = rx.recv().await {
                let node = node.clone();
                let res = exec
                    .run(|| {
                        let node = node.clone();
                        let ctx = ctx.clone();
                        let input = input.clone();
                        async move { node.execute(&ctx, input).await }
                    })
                    .await;
                let _ = reply.send(res);
            }
        });
        Self { tx }
    }

    pub async fn call(&self, input: NodeInput, ctx: NodeContext) -> KernelResult<NodeOutput> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Msg { input, ctx, reply })
            .await
            .map_err(|_| kernelflow_core::KernelError::Network("actor mailbox closed".into()))?;
        rx.await
            .map_err(|_| kernelflow_core::KernelError::Network("actor reply dropped".into()))?
    }
}
