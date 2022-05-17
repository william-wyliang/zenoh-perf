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
use clap::Parser;
use slab::Slab;
use std::{
    any::Any,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use zenoh::{
    config::{Config, WhatAmI},
    net::{
        link::{EndPoint, Link},
        protocol::proto::ZenohMessage,
        transport::*,
    },
};
use zenoh_core::zresult::ZResult;

type Table = Arc<RwLock<Slab<TransportUnicast>>>;

// Transport Handler for the peer
struct MySH {
    table: Table,
}

impl MySH {
    fn new() -> Self {
        Self {
            table: Arc::new(RwLock::new(Slab::new())),
        }
    }
}

impl TransportEventHandler for MySH {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        let index = self.table.write().unwrap().insert(transport);
        Ok(Arc::new(MyMH::new(self.table.clone(), index)))
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
    table: Table,
    index: usize,
}

impl MyMH {
    fn new(table: Table, index: usize) -> Self {
        Self { table, index }
    }
}

impl TransportPeerEventHandler for MyMH {
    fn handle_message(&self, message: ZenohMessage) -> ZResult<()> {
        for (i, e) in self.table.read().unwrap().iter() {
            if i != self.index {
                let _ = e.handle_message(message.clone());
            }
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
#[clap(name = "t_router_thr")]
struct Opt {
    /// which endpoints to listen on. e.g. --listen tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, value_delimiter = ',')]
    listen: Vec<EndPoint>,

    /// which endpoints to connect to., e.g. --connect tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, value_delimiter = ',')]
    connect: Vec<EndPoint>,

    /// configuration file (json5 or yaml)
    #[clap(long = "conf", parse(from_os_str))]
    config: Option<PathBuf>,
}

#[async_std::main]
async fn main() {
    // Parse the args
    let Opt {
        listen,
        connect,
        config,
    } = Opt::parse();

    // Create the session manager
    let builder = match config {
        Some(path) => TransportManager::builder()
            .from_config(&Config::from_file(path).unwrap())
            .await
            .unwrap(),
        None => TransportManager::builder().whatami(WhatAmI::Router),
    };
    let handler = Arc::new(MySH::new());
    let manager = builder.build(handler).unwrap();

    // Create listeners
    for l in listen {
        manager.add_listener(l.clone()).await.unwrap();
    }
    // Connect to other routers
    for l in connect {
        let _t = manager.open_transport_unicast(l.clone()).await.unwrap();
    }
    // Stop forever
    future::pending::<()>().await;
}
