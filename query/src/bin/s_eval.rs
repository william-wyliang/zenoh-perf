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
use std::path::PathBuf;
use structopt::StructOpt;
use zenoh::net::link::{EndPoint, Link};
use zenoh::net::protocol::core::{
    whatami, Channel, CongestionControl, Priority, Reliability, ResKey,
};
use zenoh::net::protocol::io::ZBuf;
use zenoh::net::protocol::proto::{Query, ReplyContext, ZenohBody, ZenohMessage};
use zenoh::net::transport::*;
use zenoh_util::core::ZResult;
use zenoh_util::properties::{IntKeyProperties, Properties};

// Transport Handler for the peer
struct MySH {
    payload: usize,
}

impl MySH {
    fn new(payload: usize) -> Self {
        Self { payload }
    }
}

impl TransportEventHandler for MySH {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        Ok(Arc::new(MyMH::new(transport, self.payload)))
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
    payload: usize,
}

impl MyMH {
    fn new(session: TransportUnicast, payload: usize) -> Self {
        Self { session, payload }
    }
}

impl TransportPeerEventHandler for MyMH {
    fn handle_message(&self, message: ZenohMessage) -> ZResult<()> {
        match message.body {
            ZenohBody::Query(Query { qid, .. }) => {
                // Send reliable messages
                let channel = Channel {
                    priority: Priority::Data,
                    reliability: Reliability::Reliable,
                };
                let congestion_control = CongestionControl::Block;
                let key = ResKey::RName("/test/query".to_string());
                let info = None;
                let payload = ZBuf::from(vec![0u8; self.payload]);
                let routing_context = None;
                let reply_context = Some(ReplyContext { qid, replier: None });
                let attachment = None;

                let message = ZenohMessage::make_data(
                    key,
                    payload,
                    channel,
                    congestion_control,
                    info,
                    routing_context,
                    reply_context,
                    attachment,
                );

                self.session.handle_message(message)
            }
            _ => panic!("Invalid message"),
        }
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
#[structopt(name = "s_eval")]
struct Opt {
    #[structopt(short = "l", long = "locator")]
    locator: EndPoint,
    #[structopt(short = "m", long = "mode")]
    mode: String,
    #[structopt(short = "p", long = "payload")]
    payload: usize,
    #[structopt(short = "c", long = "conf", parse(from_os_str))]
    config: Option<PathBuf>,
}

#[async_std::main]
async fn main() {
    // Enable logging
    env_logger::init();

    // Parse the args
    let opt = Opt::from_args();

    let whatami = whatami::parse(opt.mode.as_str()).unwrap();

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
        None => TransportManagerConfig::builder().whatami(whatami),
    };
    let config = bc.build(Arc::new(MySH::new(opt.payload)));
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
