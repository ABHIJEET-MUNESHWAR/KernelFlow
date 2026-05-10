//! # kernelflow-attest
//!
//! Signs a workflow's deterministic output hash with an ed25519 key and
//! exposes a [`SolanaAttestor`] stub that would forward the signed
//! attestation to an Anchor program (see `anchor/programs/kernelflow-attest`).

use async_trait::async_trait;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kernelflow_core::{KernelError, KernelResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub workflow_id:  uuid::Uuid,
    pub output_hash:  String,
    pub signer_pubkey_hex: String,
    pub signature_hex: String,
    pub timestamp:    chrono::DateTime<chrono::Utc>,
}

pub struct Attestor {
    key: SigningKey,
}

impl Attestor {
    pub fn random() -> Self {
        let mut rng = rand::rngs::OsRng;
        Self { key: SigningKey::generate(&mut rng) }
    }
    pub fn pubkey(&self) -> VerifyingKey { self.key.verifying_key() }
    pub fn sign(&self, workflow_id: uuid::Uuid, output_hash: String) -> Attestation {
        let msg = format!("{workflow_id}|{output_hash}");
        let sig: Signature = self.key.sign(msg.as_bytes());
        Attestation {
            workflow_id,
            output_hash,
            signer_pubkey_hex: hex::encode(self.pubkey().to_bytes()),
            signature_hex:     hex::encode(sig.to_bytes()),
            timestamp:         chrono::Utc::now(),
        }
    }
}

#[async_trait]
pub trait OnchainSubmitter: Send + Sync {
    async fn submit(&self, att: &Attestation) -> KernelResult<String>;
}

/// Stubbed Solana submitter — wired up to a real RPC by setting `rpc_url`.
pub struct SolanaAttestor { pub rpc_url: String }

#[async_trait]
impl OnchainSubmitter for SolanaAttestor {
    async fn submit(&self, att: &Attestation) -> KernelResult<String> {
        // In production: build, sign, send a Solana tx that invokes the
        // `kernelflow-attest` Anchor program. Here we POST to the RPC for
        // a `getHealth` smoke test so the integration path is exercised.
        let client = reqwest::Client::new();
        let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"getHealth"});
        let res = client.post(&self.rpc_url).json(&body).send().await
            .map_err(|e| KernelError::Attestation(e.to_string()))?;
        if !res.status().is_success() {
            return Err(KernelError::Attestation(format!("rpc {}", res.status())));
        }
        Ok(format!("simulated-tx-for-{}", att.output_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sign_verify_roundtrip() {
        let a = Attestor::random();
        let att = a.sign(uuid::Uuid::nil(), "deadbeef".into());
        let pk = VerifyingKey::from_bytes(
            &<[u8; 32]>::try_from(hex::decode(att.signer_pubkey_hex).unwrap().as_slice()).unwrap()
        ).unwrap();
        let sig = Signature::from_bytes(
            &<[u8; 64]>::try_from(hex::decode(att.signature_hex).unwrap().as_slice()).unwrap()
        );
        let msg = format!("{}|{}", att.workflow_id, att.output_hash);
        assert!(pk.verify_strict(msg.as_bytes(), &sig).is_ok());
    }
}

