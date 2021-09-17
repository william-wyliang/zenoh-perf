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
use std::any::Any;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use structopt::StructOpt;
use zenoh::net::link::{EndPoint, Link};
use zenoh::net::protocol::core::{
    whatami, Channel, CongestionControl, Priority, Reliability, ResKey,
};
use zenoh::net::protocol::io::ZBuf;
use zenoh::net::protocol::proto::ZenohMessage;
use zenoh::net::transport::{
    TransportEventHandler, TransportManager, TransportManagerConfig, TransportMulticast,
    TransportMulticastEventHandler, TransportPeer, TransportPeerEventHandler, TransportUnicast,
};
use zenoh_util::core::ZResult;
use zenoh_util::properties::{IntKeyProperties, Properties};

// Transport Handler for the peer
struct MySH {
    scenario: String,
    name: String,
    payload: usize,
    counter: Arc<AtomicUsize>,
    active: AtomicBool,
}

impl MySH {
    fn new(scenario: String, name: String, payload: usize, counter: Arc<AtomicUsize>) -> Self {
        Self {
            scenario,
            name,
            payload,
            counter,
            active: AtomicBool::new(false),
        }
    }
}

impl TransportEventHandler for MySH {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        _transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        if !self.active.swap(true, Ordering::Acquire) {
            let count = self.counter.clone();
            let scenario = self.scenario.clone();
            let name = self.name.clone();
            let payload = self.payload;
            task::spawn(async move {
                loop {
                    task::sleep(Duration::from_secs(1)).await;
                    let c = count.swap(0, Ordering::Relaxed);
                    println!("session,{},throughput,{},{},{}", scenario, name, payload, c);
                }
            });
        }
        Ok(Arc::new(MyMH::new(self.counter.clone())))
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
    counter: Arc<AtomicUsize>,
}

impl MyMH {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        Self { counter }
    }
}

impl TransportPeerEventHandler for MyMH {
    fn handle_message(&self, _message: ZenohMessage) -> ZResult<()> {
        self.counter.fetch_add(1, Ordering::Relaxed);
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
#[structopt(name = "s_pubsub_thr")]
struct Opt {
    #[structopt(short = "l", long = "listen")]
    listen: Vec<EndPoint>,
    #[structopt(short = "c", long = "connect")]
    connect: Vec<EndPoint>,
    #[structopt(short = "m", long = "mode")]
    mode: String,
    #[structopt(short = "p", long = "payload")]
    payload: usize,
    #[structopt(short = "n", long = "name")]
    name: String,
    #[structopt(short = "s", long = "scenario")]
    scenario: String,
    #[structopt(short = "t", long = "print")]
    print: bool,
    #[structopt(long = "conf", parse(from_os_str))]
    config: Option<PathBuf>,
}

#[async_std::main]
async fn main() {
    // Enable logging
    env_logger::init();

    // Parse the args
    let opt = Opt::from_args();

    let whatami = whatami::parse(opt.mode.as_str()).unwrap();

    let count = Arc::new(AtomicUsize::new(0));
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
    let config = bc.build(Arc::new(MySH::new(
        opt.scenario,
        opt.name,
        opt.payload,
        count,
    )));
    let manager = TransportManager::new(config);

    // Connect to publisher
    for e in opt.listen.iter() {
        let _ = manager.add_listener(e.clone()).await.unwrap();
    }

    let mut transports: Vec<TransportUnicast> = vec![];
    for e in opt.connect.iter() {
        let t = loop {
            match manager.open_transport_unicast(e.clone()).await {
                Ok(t) => break t,
                Err(_) => task::sleep(Duration::from_secs(1)).await,
            }
        };
        transports.push(t);
    }

    // Send reliable messages
    let channel = Channel {
        priority: Priority::Data,
        reliability: Reliability::Reliable,
    };
    let congestion_control = CongestionControl::Block;
    let key = ResKey::RName("test".to_string());
    let info = None;
    let payload = ZBuf::from(vec![0u8; opt.payload]);
    let reply_context = None;
    let routing_context = None;
    let attachment = None;

    let count = Arc::new(AtomicUsize::new(0));
    if opt.print {
        let c_count = count.clone();
        task::spawn(async move {
            loop {
                task::sleep(Duration::from_secs(1)).await;
                let c = c_count.swap(0, Ordering::Relaxed);
                if c > 0 {
                    println!("{} msg/s", c);
                }
            }
        });
    }

    loop {
        for t in transports.iter() {
            let message = ZenohMessage::make_data(
                key.clone(),
                payload.clone(),
                channel,
                congestion_control,
                info.clone(),
                routing_context,
                reply_context.clone(),
                attachment.clone(),
            );
            let _ = t.handle_message(message).unwrap();
        }
        count.fetch_add(1, Ordering::Relaxed);
    }
}
