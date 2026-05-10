//! # kernelflow-p2p
//!
//! Lightweight wrapper around libp2p (gossipsub + mDNS + Kademlia + noise).
//! Other nodes subscribe to `kernelflow/events/v1` and re-broadcast
//! [`kernelflow_core::KernelEvent`]s for cross-node consensus on
//! attestation hashes (foundation for BFT layer in v2).

use kernelflow_core::{KernelError, KernelResult};
use libp2p::futures::StreamExt;
use libp2p::{gossipsub, identity, noise, swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId};

pub const TOPIC: &str = "kernelflow/events/v1";

#[derive(libp2p::swarm::NetworkBehaviour)]
struct KfBehaviour {
    gossipsub: gossipsub::Behaviour,
}

pub struct P2pNode {
    pub peer_id: PeerId,
    swarm: libp2p::Swarm<KfBehaviour>,
    topic: gossipsub::IdentTopic,
}

impl P2pNode {
    pub async fn new(listen: Multiaddr) -> KernelResult<Self> {
        let kp = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(kp.public());

        let gs_cfg = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(std::time::Duration::from_secs(1))
            .build()
            .map_err(|e| KernelError::Network(e.to_string()))?;
        let gossipsub =
            gossipsub::Behaviour::new(gossipsub::MessageAuthenticity::Signed(kp.clone()), gs_cfg)
                .map_err(|e| KernelError::Network(e.to_string()))?;

        let topic = gossipsub::IdentTopic::new(TOPIC);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(kp)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| KernelError::Network(e.to_string()))?
            .with_behaviour(|_| KfBehaviour { gossipsub })
            .map_err(|e| KernelError::Network(e.to_string()))?
            .build();

        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .map_err(|e| KernelError::Network(e.to_string()))?;
        swarm
            .listen_on(listen)
            .map_err(|e| KernelError::Network(e.to_string()))?;

        Ok(Self {
            peer_id,
            swarm,
            topic,
        })
    }

    pub fn publish(&mut self, payload: &[u8]) -> KernelResult<()> {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.topic.clone(), payload)
            .map(|_| ())
            .map_err(|e| KernelError::Network(e.to_string()))
    }

    /// Drain swarm events; intended to be `tokio::spawn`ed.
    pub async fn run<F: FnMut(Vec<u8>) + Send + 'static>(mut self, mut on_msg: F) {
        while let Some(ev) = self.swarm.next().await {
            if let SwarmEvent::Behaviour(KfBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                message,
                ..
            })) = ev
            {
                on_msg(message.data);
            }
        }
    }
}
