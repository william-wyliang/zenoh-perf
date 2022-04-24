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
use async_std::future;
use async_std::sync::Arc;
use std::any::Any;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use clap::Parser;
use zenoh::net::link::{EndPoint, Link};
use zenoh::net::protocol::core::whatami;
use zenoh::net::protocol::proto::{Data, ZenohBody, ZenohMessage};
use zenoh::net::transport::*;
use zenoh_util::core::ZResult;

// Transport Handler for the peer
struct MySH;

impl MySH {
    fn new() -> Self {
        Self
    }
}

impl TransportEventHandler for MySH {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        _transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        Ok(Arc::new(MyMH::new()))
    }

    fn new_multicast(
        &self,
        _transport: TransportMulticast,
    ) -> ZResult<Arc<dyn TransportMulticastEventHandler>> {
        panic!();
    }
}

// Message Handler for the peer
struct MyMH;

impl MyMH {
    fn new() -> Self {
        Self
    }
}

impl TransportPeerEventHandler for MyMH {
    fn handle_message(&self, message: ZenohMessage) -> ZResult<()> {
        match message.body {
            ZenohBody::Data(Data { mut payload, .. }) => {
                let mut count_bytes = [0u8; 8];
                payload.read_bytes(&mut count_bytes);
                let count = u64::from_le_bytes(count_bytes);

                let mut now_bytes = [0u8; 16];
                payload.read_bytes(&mut now_bytes);
                let now_pub = u128::from_le_bytes(now_bytes);

                let now_sub = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let interval = Duration::from_nanos((now_sub - now_pub) as u64);

                println!("{} bytes: seq={} time={:?}", payload.len(), count, interval);
            }
            _ => panic!("Invalid message"),
        }

        Ok(())
    }

    fn new_link(&self, _link: Link) {}
    fn del_link(&self, _link: Link) {}
    fn closing(&self) {}
    fn closed(&self) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Parser)]
#[clap(name = "t_sub_delay")]
struct Opt {
    /// endpoint, e.g. --endpoint tcp/127.0.0.1:7447
    #[clap(short, long)]
    endpoint: String,

    /// peer or client or router
    #[clap(short, long)]
    mode: String,
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

    // Connect to the peer or listen
    if whatami == whatami::PEER {
        manager.add_listener(opt.locator).await.unwrap();
    } else {
        let _session = manager.open_transport(opt.locator).await.unwrap();
    }

    // Stop forever
    future::pending::<()>().await;
}
