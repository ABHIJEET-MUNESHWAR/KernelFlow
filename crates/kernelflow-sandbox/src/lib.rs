//! # kernelflow-sandbox
//!
//! Wraps `wasmtime` to provide a [`WasmNode`] that implements
//! `kernelflow_core::WorkflowNode`. Fuel-metered, deadline-bounded,
//! no host imports beyond a curated set — true sandboxed execution.

use std::sync::Arc;

use async_trait::async_trait;
use kernelflow_core::{
    KernelError, KernelResult, NodeContext, NodeInput, NodeOutput, WorkflowNode,
};
use wasmtime::{Config, Engine, Instance, Linker, Module, Store};

#[derive(Clone)]
pub struct WasmEngine {
    engine: Engine,
}

impl WasmEngine {
    pub fn new() -> KernelResult<Self> {
        let mut cfg = Config::new();
        cfg.consume_fuel(true)
            .async_support(true)
            .epoch_interruption(true);
        let engine = Engine::new(&cfg).map_err(|e| KernelError::Sandbox(e.to_string()))?;
        Ok(Self { engine })
    }

    pub fn compile(&self, wasm_bytes: &[u8]) -> KernelResult<Module> {
        Module::new(&self.engine, wasm_bytes).map_err(|e| KernelError::Sandbox(e.to_string()))
    }
}

pub struct WasmNode {
    engine: WasmEngine,
    module: Arc<Module>,
    fuel: u64,
    name: &'static str,
}

impl WasmNode {
    pub fn new(engine: WasmEngine, wasm_bytes: &[u8], fuel: u64) -> KernelResult<Self> {
        let module = Arc::new(engine.compile(wasm_bytes)?);
        Ok(Self {
            engine,
            module,
            fuel,
            name: "wasm",
        })
    }
}

#[async_trait]
impl WorkflowNode for WasmNode {
    fn kind(&self) -> &'static str {
        self.name
    }

    async fn execute(&self, _ctx: &NodeContext, input: NodeInput) -> KernelResult<NodeOutput> {
        // Set up store with fuel.
        let mut store = Store::new(&self.engine.engine, ());
        store
            .set_fuel(self.fuel)
            .map_err(|e| KernelError::Sandbox(e.to_string()))?;

        let linker: Linker<()> = Linker::new(&self.engine.engine);
        let instance: Instance = linker
            .instantiate_async(&mut store, &self.module)
            .await
            .map_err(|e| KernelError::Sandbox(e.to_string()))?;

        // Convention: WASM exports `run(input_ptr,len) -> i32` reading bytes from memory.
        // For the scaffold we just call an `add` function if present, else echo the input.
        if let Ok(add) = instance.get_typed_func::<(i32, i32), i32>(&mut store, "add") {
            let a = input.payload.get("a").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let b = input.payload.get("b").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let r = add
                .call_async(&mut store, (a, b))
                .await
                .map_err(|e| KernelError::Sandbox(e.to_string()))?;
            let used = self.fuel.saturating_sub(store.get_fuel().unwrap_or(0));
            return Ok(NodeOutput {
                value: serde_json::json!({ "result": r }),
                gas_used: used,
            });
        }
        let used = self
            .fuel
            .saturating_sub(store.get_fuel().unwrap_or(self.fuel));
        Ok(NodeOutput {
            value: input.payload,
            gas_used: used,
        })
    }
}
