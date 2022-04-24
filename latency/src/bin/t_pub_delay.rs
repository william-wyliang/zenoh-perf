//
// Copyright (c) 2017, 2020 ADLINK Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ADLINK zenoh team, <zenoh@adlink-labs.tech>
//
use async_std::sync::Arc;
use async_std::task;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use clap::Parser;
use zenoh::net::link::EndPoint;
use zenoh::net::protocol::core::{
    whatami, Channel, CongestionControl, Priority, Reliability, ResKey,
};
use zenoh::net::protocol::proto::ZenohMessage;
use zenoh::net::transport::{
    DummyTransportPeerEventHandler, TransportEventHandler, TransportManager,
    TransportManagerConfig, TransportMulticast, TransportMulticastEventHandler, TransportPeer,
    TransportPeerEventHandler, TransportUnicast,
};
use zenoh_util::core::ZResult;

struct MySH {}

impl MySH {
    fn new() -> Self {
        Self {}
    }
}

impl TransportEventHandler for MySH {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        _transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        Ok(Arc::new(DummyTransportPeerEventHandler::default()))
    }

    fn new_multicast(
        &self,
        _transport: TransportMulticast,
    ) -> ZResult<Arc<dyn TransportMulticastEventHandler>> {
        panic!();
    }
}

#[derive(Debug, Parser)]
#[clap(name = "t_pub_delay")]
struct Opt {
    /// endpoint, e.g. --endpoint tcp/127.0.0.1:7447
    #[clap(short, long)]
    endpoint: String,

    /// peer or client or router
    #[clap(short, long)]
    mode: String,

    /// payload size (bytes)
    #[clap(short, long)]
    payload: usize,

    /// interval of sending message (sec)
    #[clap(short, long)]
    interval: f64,
}

#[async_std::main]
async fn main() {
    // Enable logging
    env_logger::init();

    // Parse the args
    let opt = Opt::parse();

    let whatami = whatami::parse(opt.mode.as_str()).unwrap();

    let config = TransportManagerConfig::builder()
        .whatami(whatami)
        .build(Arc::new(MySH::new()));
    let manager = TransportManager::new(config);

    // Connect to publisher
    let session = manager.open_transport(opt.locator).await.unwrap();

    let mut count: u64 = 0;
    loop {
        // Send reliable messages
        let channel = Channel {
            priority: Priority::Data,
            reliability: Reliability::Reliable,
        };
        let congestion_control = CongestionControl::Block;
        let key = ResKey::RName("/test/ping".to_string());
        let info = None;
        let routing_context = None;
        let reply_context = None;
        let attachment = None;

        // u64 (8 bytes) for seq num
        // u128 (16 bytes) for system time in nanoseconds
        let mut payload = vec![0u8; opt.payload];
        let count_bytes: [u8; 8] = count.to_le_bytes();
        let now_bytes: [u8; 16] = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_le_bytes();
        payload[0..8].copy_from_slice(&count_bytes);
        payload[8..24].copy_from_slice(&now_bytes);

        let message = ZenohMessage::make_data(
            key,
            payload.into(),
            channel,
            congestion_control,
            info,
            routing_context,
            reply_context,
            attachment,
        );

        let _ = session.handle_message(message.clone()).unwrap();

        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}
