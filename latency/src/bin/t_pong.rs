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
use std::str::FromStr;
use std::any::Any;
use clap::Parser;
use zenoh::net::link::Link;
use zenoh_protocol_core::{WhatAmI, EndPoint};
use zenoh::net::protocol::proto::ZenohMessage;
use zenoh::net::transport::{
    TransportEventHandler, TransportManager, TransportMulticast,
    TransportMulticastEventHandler, TransportPeer, TransportPeerEventHandler, TransportUnicast,
};
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
        transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        Ok(Arc::new(MyMH::new(transport)))
    }

    fn new_multicast(
        &self,
        _transport: TransportMulticast,
    ) -> ZResult<Arc<dyn TransportMulticastEventHandler>> {
        panic!();
    }
}

// Message Handler for the peer
struct MyMH {
    session: TransportUnicast,
}

impl MyMH {
    fn new(session: TransportUnicast) -> Self {
        Self { session }
    }
}

impl TransportPeerEventHandler for MyMH {
    fn handle_message(&self, message: ZenohMessage) -> ZResult<()> {
        self.session.handle_message(message)
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
#[clap(name = "t_pong")]
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
        manager.add_listener(EndPoint::from_str(opt.endpoint.as_str()).unwrap()).await.unwrap();
    } else {
        let _session = manager.open_transport(EndPoint::from_str(opt.endpoint.as_str()).unwrap()).await.unwrap();
    }

    // Stop forever
    future::pending::<()>().await;
}
