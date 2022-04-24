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
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use clap::Parser;
use zenoh::net::link::Link;
use zenoh::net::protocol::io::reader::{HasReader, Reader};
use zenoh::net::protocol::io::SplitBuffer;
use zenoh_protocol_core::{WhatAmI, EndPoint};
use zenoh::net::protocol::proto::{Data, ZenohBody, ZenohMessage};
use zenoh::net::transport::*;
use zenoh_core::Result as ZResult;

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
            ZenohBody::Data(Data { payload, .. }) => {
                let mut count_bytes = [0u8; 8];
                let mut now_bytes = [0u8; 16];

                let mut data_reader = payload.reader();
                if data_reader.read_exact(&mut count_bytes) && data_reader.read_exact(&mut now_bytes) {
                    let count = u64::from_le_bytes(count_bytes);
                    let now_pub = u128::from_le_bytes(now_bytes);

                    let now_sub = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos();
                    let interval = Duration::from_nanos((now_sub - now_pub) as u64);

                    println!("{} bytes: seq={} time={:?}", payload.len(), count, interval);
                } else {
                    panic!("Fail to fill the buffer");
                }
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

    let whatami = WhatAmI::from_str(opt.mode.as_str()).unwrap();
  

    let manager = TransportManager::builder()
        .whatami(whatami)
        .build(Arc::new(MySH::new()))
        .unwrap();

    // Connect to the peer or listen
    if whatami == WhatAmI::Peer {
        manager
            .add_listener(EndPoint::from_str(opt.endpoint.as_str()).unwrap())
            .await
            .unwrap();
    } else {
        let _session = manager
            .open_transport(EndPoint::from_str(opt.endpoint.as_str()).unwrap())
            .await
            .unwrap(); 
    }

    // Stop forever
    future::pending::<()>().await;
}
