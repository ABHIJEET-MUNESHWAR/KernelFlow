//! # kernelflow-storage
//!
//! Embedded RocksDB store. Uses column families as a form of **partitioning**:
//!   * `events`     — append-only event log keyed by `(workflow_id, seq)`
//!   * `state`      — workflow state snapshots keyed by `workflow_id`
//!   * `attestations` — signed attestation records keyed by hash
//!
//! For horizontal **sharding** the `Store` opens N RocksDB databases under
//! different paths and routes by `hash(workflow_id) % N` (see [`ShardedStore`]).
//!
//! Async-friendly: blocking RocksDB calls are wrapped in `tokio::task::spawn_blocking`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kernelflow_core::{KernelError, KernelResult};
use rocksdb::{ColumnFamilyDescriptor, DB, Options};
use serde::{de::DeserializeOwned, Serialize};

const CF_EVENTS:       &str = "events";
const CF_STATE:        &str = "state";
const CF_ATTESTATIONS: &str = "attestations";

pub struct Store { db: Arc<DB> }

impl Store {
    pub fn open(path: impl AsRef<Path>) -> KernelResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_EVENTS,       Options::default()),
            ColumnFamilyDescriptor::new(CF_STATE,        Options::default()),
            ColumnFamilyDescriptor::new(CF_ATTESTATIONS, Options::default()),
        ];
        let db = DB::open_cf_descriptors(&opts, path, cfs)
            .map_err(|e| KernelError::Storage(e.to_string()))?;
        Ok(Self { db: Arc::new(db) })
    }

    pub async fn put<V: Serialize + Send + 'static>(&self, cf: Cf, key: Vec<u8>, val: V) -> KernelResult<()> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let bytes = bincode::serialize(&val).map_err(|e| KernelError::Storage(e.to_string()))?;
            let h = db.cf_handle(cf.as_str()).ok_or_else(|| KernelError::Storage("cf missing".into()))?;
            db.put_cf(&h, key, bytes).map_err(|e| KernelError::Storage(e.to_string()))
        }).await.map_err(|e| KernelError::Storage(e.to_string()))?
    }

    pub async fn get<V: DeserializeOwned + Send + 'static>(&self, cf: Cf, key: Vec<u8>) -> KernelResult<Option<V>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let h = db.cf_handle(cf.as_str()).ok_or_else(|| KernelError::Storage("cf missing".into()))?;
            let v = db.get_cf(&h, key).map_err(|e| KernelError::Storage(e.to_string()))?;
            v.map(|b| bincode::deserialize::<V>(&b).map_err(|e| KernelError::Storage(e.to_string()))).transpose()
        }).await.map_err(|e| KernelError::Storage(e.to_string()))?
    }
}

#[derive(Copy, Clone)]
pub enum Cf { Events, State, Attestations }
impl Cf {
    fn as_str(&self) -> &'static str {
        match self { Cf::Events => CF_EVENTS, Cf::State => CF_STATE, Cf::Attestations => CF_ATTESTATIONS }
    }
}

/// Hash-sharded store: routes keys to one of N underlying RocksDB instances.
/// Enables horizontal scale-out across NVMe devices.
pub struct ShardedStore {
    shards: Vec<Store>,
}

impl ShardedStore {
    pub fn open(base: impl AsRef<Path>, shards: usize) -> KernelResult<Self> {
        let base: PathBuf = base.as_ref().to_path_buf();
        let mut v = Vec::with_capacity(shards);
        for i in 0..shards {
            v.push(Store::open(base.join(format!("shard-{i}")))?);
        }
        Ok(Self { shards: v })
    }
    pub fn shard_for(&self, key: &[u8]) -> &Store {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        &self.shards[(h.finish() as usize) % self.shards.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        s.put(Cf::State, b"k".to_vec(), 42u32).await.unwrap();
        let v: Option<u32> = s.get(Cf::State, b"k".to_vec()).await.unwrap();
        assert_eq!(v, Some(42));
    }
}

