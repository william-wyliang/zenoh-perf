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
use async_std::{sync::Arc, task};
use clap::Parser;
use std::{
    any::Any,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};
use zenoh::{
    config::{Config, WhatAmI},
    net::{
        link::{EndPoint, Link},
        protocol::core::{Channel, CongestionControl, Priority, Reliability},
        protocol::{io::ZBuf, proto::ZenohMessage},
        transport::{
            TransportEventHandler, TransportManager, TransportMulticast,
            TransportMulticastEventHandler, TransportPeer, TransportPeerEventHandler,
            TransportUnicast,
        },
    },
    prelude::KeyExpr,
};
use zenoh_core::zresult::ZResult;

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

#[derive(Debug, Parser)]
#[clap(name = "t_pubsub_thr")]
struct Opt {
    /// which endpoints to listen on. e.g. --listen tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, value_delimiter = ',')]
    listen: Vec<EndPoint>,

    /// which endpoints to connect to., e.g. --connect tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, value_delimiter = ',')]
    connect: Vec<EndPoint>,

    /// peer, router, or client
    #[clap(short, long)]
    mode: WhatAmI,

    /// payload size (bytes)
    #[clap(short, long)]
    payload: usize,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    scenario: String,

    /// print the counter
    #[clap(short = 't', long)]
    print: bool,

    /// configuration file (json5 or yaml)
    #[clap(long = "conf", parse(from_os_str))]
    config: Option<PathBuf>,
}

#[async_std::main]
async fn main() {
    // Enable logging
    env_logger::init();

    // Parse the args
    let Opt {
        listen,
        connect,
        mode,
        payload,
        name,
        scenario,
        print,
        config,
    } = Opt::parse();

    let count = Arc::new(AtomicUsize::new(0));
    let builder = match config {
        Some(path) => TransportManager::builder()
            .from_config(&Config::from_file(path).unwrap())
            .await
            .unwrap(),
        None => TransportManager::builder().whatami(mode),
    };
    let handler = Arc::new(MySH::new(scenario, name, payload, count));
    let manager = builder.build(handler).unwrap();

    if listen.is_empty() && connect.is_empty() {
        panic!("Either --listen or --connect needs to be specified, see --help for more details");
    }

    // Connect to publisher
    for e in listen {
        let _ = manager.add_listener(e.clone()).await.unwrap();
    }

    let mut transports: Vec<TransportUnicast> = vec![];
    for e in connect {
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
    let key = KeyExpr::from("test");
    let info = None;
    let payload = ZBuf::from(vec![0u8; payload]);
    let reply_context = None;
    let routing_context = None;
    let attachment = None;

    let count = Arc::new(AtomicUsize::new(0));
    if print {
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
            t.handle_message(message).unwrap();
        }
        count.fetch_add(1, Ordering::Relaxed);
    }
}
