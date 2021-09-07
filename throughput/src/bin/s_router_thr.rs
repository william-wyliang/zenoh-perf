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
use rand::RngCore;
use slab::Slab;
use std::any::Any;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use structopt::StructOpt;
use zenoh::net::link::{EndPoint, Link};
use zenoh::net::protocol::core::{whatami, PeerId};
use zenoh::net::protocol::proto::ZenohMessage;
use zenoh::net::transport::*;
use zenoh_util::core::ZResult;
use zenoh_util::properties::{IntKeyProperties, Properties};

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

#[derive(Debug, StructOpt)]
#[structopt(name = "s_router_thr")]
struct Opt {
    #[structopt(short = "l", long = "locator")]
    locator: Vec<EndPoint>,
    #[structopt(short = "c", long = "conf", parse(from_os_str))]
    config: Option<PathBuf>,
}

#[async_std::main]
async fn main() {
    // Parse the args
    let mut opt = Opt::from_args();

    // Initialize the Peer Id
    let mut pid = [0u8; PeerId::MAX_SIZE];
    rand::thread_rng().fill_bytes(&mut pid);
    let pid = PeerId::new(1, pid);

    // Create the session manager
    let bc = match opt.config.as_ref() {
        Some(f) => {
            let config = async_std::fs::read_to_string(f).await.unwrap();
            let properties = Properties::from(config);
            let int_props = IntKeyProperties::from(properties);
            TransportManagerConfig::builder()
                .from_config(&int_props)
                .await
                .unwrap()
        }
        None => TransportManagerConfig::builder()
            .whatami(whatami::ROUTER)
            .pid(pid),
    };
    let config = bc.build(Arc::new(MySH::new()));
    let manager = TransportManager::new(config);

    // Connect to publisher
    for l in opt.locator.drain(..) {
        manager.add_listener(l).await.unwrap();
    }
    // Stop forever
    future::pending::<()>().await;
}
