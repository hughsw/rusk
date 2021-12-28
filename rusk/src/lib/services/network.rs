// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Public Key infrastructure service implementation for the Rusk server.

use kadcast::{MessageInfo, NetworkListen, Peer};
use tokio::sync::broadcast::{self, error::RecvError, Sender};
use tonic::{Request, Response, Status};
use tracing::{error, trace, warn};

pub use super::rusk_proto::{
    network_server::{Network, NetworkServer},
    BroadcastMessage, Message, MessageMetadata, Null, SendMessage,
};
use futures::Stream;
use std::time::Duration;
use std::{net::SocketAddr, pin::Pin};

pub struct RuskNetwork {
    peer: Peer,
    sender: Sender<(Vec<u8>, SocketAddr, u8)>,
}

impl RuskNetwork {
    pub fn new(
        public_addr: String,
        listen_addr: Option<String>,
        bootstrap: Vec<String>,
    ) -> RuskNetwork {
        // Creating a broadcast channel which each grpc `listen` calls will
        // listen to.
        // The sender is used by the KadcastListener to forward the received
        // messages.
        // The receiver is discarded because at the moment 0 there is no one
        // listening.
        // When a `listen` call is received, a new receiver is created using
        // `sender.subscribe`
        let grpc_sender = broadcast::channel(100).0;
        let listener = KadcastListener {
            grpc_sender: grpc_sender.clone(),
        };
        let mut peer_builder = Peer::builder(public_addr, bootstrap, listener)
            .with_listen_address(listen_addr)
            .with_node_ttl(Duration::from_millis(30_000))
            .with_bucket_ttl(Duration::from_secs(60 * 60))
            .with_recursive_discovery(false) //Default is true
            .with_channel_size(100)
            .with_node_evict_after(Duration::from_millis(5_000))
            .with_auto_propagate(true);
        //this is unusefull, just to get the default conf
        peer_builder
            .transport_conf()
            .extend(kadcast::transport::default_configuration());

        //RaptorQ Decoder conf
        peer_builder
            .transport_conf()
            .insert("cache_ttl_secs".to_string(), "60".to_string());
        peer_builder
            .transport_conf()
            .insert("cache_prune_every_secs".to_string(), "300".to_string());

        //RaptorQ Encoder conf
        peer_builder.transport_conf().insert(
            "min_repair_packets_per_block".to_string(),
            "5".to_string(),
        );
        peer_builder
            .transport_conf()
            .insert("mtu".to_string(), "1400".to_string());

        RuskNetwork {
            peer: peer_builder.build(),
            sender: grpc_sender,
        }
    }
}

impl Default for RuskNetwork {
    fn default() -> RuskNetwork {
        RuskNetwork::new("127.0.0.1:9999".to_string(), None, vec![])
    }
}
struct KadcastListener {
    grpc_sender: broadcast::Sender<(Vec<u8>, SocketAddr, u8)>,
}

impl NetworkListen for KadcastListener {
    fn on_message(&self, message: Vec<u8>, metadata: MessageInfo) {
        self.grpc_sender
            .send((message, metadata.src(), metadata.height()))
            .unwrap_or_else(|e| {
                println!("Error {}", e);
                0
            });
    }
}

#[tonic::async_trait]
impl Network for RuskNetwork {
    async fn send(
        &self,
        request: Request<SendMessage>,
    ) -> Result<Response<Null>, Status> {
        trace!("Recieved SendMessage request");
        self.peer
            .send(
                &request.get_ref().message,
                request.get_ref().target_address.parse().map_err(|_| {
                    Status::invalid_argument("Unable to parse address")
                })?,
            )
            .await;
        Ok(Response::new(Null {}))
    }

    async fn broadcast(
        &self,
        request: Request<BroadcastMessage>,
    ) -> Result<Response<Null>, Status> {
        trace!("Recieved BroadcastMessage request");
        self.peer
            .broadcast(
                &request.get_ref().message,
                Some(request.get_ref().kadcast_height as usize),
            )
            .await;
        Ok(Response::new(Null {}))
    }

    type ListenStream =
        Pin<Box<dyn Stream<Item = Result<Message, Status>> + Send + 'static>>;

    async fn listen(
        &self,
        _: Request<Null>,
    ) -> Result<Response<Self::ListenStream>, Status> {
        trace!("Recieved Listen request");
        let mut rx = self.sender.subscribe();
        let output = async_stream::try_stream! {
            loop {
                match rx.recv().await {
                    Ok((message, source_address, k_height)) => {
                        yield Message {
                            message,
                            metadata: Some(MessageMetadata {
                                src_address: source_address.to_string(),
                                kadcast_height: k_height as u32,
                            }),
                        }
                    }
                    Err(e) => match e {
                        RecvError::Closed => {
                            error!("Sender stream is closed");
                            return;
                        },
                        RecvError::Lagged(skipped) => warn!("Skipped {} message", skipped)
                    }
                }
            }
        };
        Ok(Response::new(Box::pin(output) as Self::ListenStream))
    }
}
