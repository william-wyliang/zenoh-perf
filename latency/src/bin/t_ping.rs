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
use async_std::task;
use clap::Parser;
use std::any::Any;
use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};
use zenoh::net::link::Link;
use zenoh::net::protocol::io::reader::{HasReader, Reader};
use zenoh::net::protocol::io::SplitBuffer;
use zenoh::net::protocol::io::{WBuf, ZBuf};
use zenoh::net::protocol::proto::{Data, ZenohBody, ZenohMessage};
use zenoh::net::transport::*;
use zenoh_core::Result as ZResult;
use zenoh_protocol_core::{Channel, CongestionControl, EndPoint, Priority, Reliability, WhatAmI};

// Transport Handler for the non-blocking endpoint
struct MySHParallel {
    scenario: String,
    name: String,
    interval: f64,
    pending: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl MySHParallel {
    fn new(
        scenario: String,
        name: String,
        interval: f64,
        pending: Arc<Mutex<HashMap<u64, Instant>>>,
    ) -> Self {
        Self {
            scenario,
            name,
            interval,
            pending,
        }
    }
}

impl TransportEventHandler for MySHParallel {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        _transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        Ok(Arc::new(MyMHParallel::new(
            self.scenario.clone(),
            self.name.clone(),
            self.interval,
            self.pending.clone(),
        )))
    }

    fn new_multicast(
        &self,
        _transport: TransportMulticast,
    ) -> ZResult<Arc<dyn TransportMulticastEventHandler>> {
        panic!();
    }
}

// Message Handler for the endpoint
struct MyMHParallel {
    scenario: String,
    name: String,
    interval: f64,
    pending: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl MyMHParallel {
    fn new(
        scenario: String,
        name: String,
        interval: f64,
        pending: Arc<Mutex<HashMap<u64, Instant>>>,
    ) -> Self {
        Self {
            scenario,
            name,
            interval,
            pending,
        }
    }
}

impl TransportPeerEventHandler for MyMHParallel {
    fn handle_message(&self, message: ZenohMessage) -> ZResult<()> {
        match message.body {
            ZenohBody::Data(Data { payload, .. }) => {
                let mut count_bytes = [0u8; 8];
                let mut data_reader = payload.reader();
                if data_reader.read_exact(&mut count_bytes) {
                    let count = u64::from_le_bytes(count_bytes);
                    let instant = self.pending.lock().unwrap().remove(&count).unwrap();
                    println!(
                        "session,{},latency.parallel,{},{},{},{},{}",
                        self.scenario,
                        self.name,
                        payload.len(),
                        self.interval,
                        count,
                        instant.elapsed().as_micros()
                    );
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

// Transport Handler for the blocking endpoint
struct MySHSequential {
    pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>>,
}

impl MySHSequential {
    fn new(pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>>) -> Self {
        Self { pending }
    }
}

impl TransportEventHandler for MySHSequential {
    fn new_unicast(
        &self,
        _peer: TransportPeer,
        _transport: TransportUnicast,
    ) -> ZResult<Arc<dyn TransportPeerEventHandler>> {
        Ok(Arc::new(MyMHSequential::new(self.pending.clone())))
    }

    fn new_multicast(
        &self,
        _transport: TransportMulticast,
    ) -> ZResult<Arc<dyn TransportMulticastEventHandler>> {
        panic!();
    }
}

// Message Handler for the endpoint
struct MyMHSequential {
    pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>>,
}

impl MyMHSequential {
    fn new(pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>>) -> Self {
        Self { pending }
    }
}

impl TransportPeerEventHandler for MyMHSequential {
    fn handle_message(&self, message: ZenohMessage) -> ZResult<()> {
        match message.body {
            ZenohBody::Data(Data { payload, .. }) => {
                let mut count_bytes = [0u8; 8];
                let mut data_reader = payload.reader();
                if data_reader.read_exact(&mut count_bytes) {
                    let count = u64::from_le_bytes(count_bytes);
                    let barrier = self.pending.lock().unwrap().remove(&count).unwrap();
                    barrier.wait();
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
#[clap(name = "t_ping")]
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

    /// name of the test
    #[clap(short, long)]
    name: String,

    /// name of the scenario
    #[clap(short, long)]
    scenario: String,

    /// interval of sending message (sec)
    #[clap(short, long)]
    interval: f64,

    /// spawn a task to receive or not
    #[clap(long)]
    parallel: bool,
}

async fn single(opt: Opt, whatami: WhatAmI) {
    let pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>> = Arc::new(Mutex::new(HashMap::new()));
    let manager = TransportManager::builder()
        .whatami(whatami)
        .build(Arc::new(MySHSequential::new(pending.clone())))
        .unwrap();

    // Connect to publisher
    let session = manager
        .open_transport(EndPoint::from_str(opt.endpoint.as_str()).unwrap())
        .await
        .unwrap();

    let sleep = Duration::from_secs_f64(opt.interval);
    let payload = vec![0u8; opt.payload - 8];
    let mut count: u64 = 0;
    loop {
        // Create and send the message
        let channel = Channel {
            priority: Priority::Data,
            reliability: Reliability::Reliable,
        };
        let congestion_control = CongestionControl::Block;
        let key = "/test/ping";
        let info = None;

        let mut data: WBuf = WBuf::new(opt.payload, true);
        let count_bytes: [u8; 8] = count.to_le_bytes();
        data.write_all(&count_bytes).unwrap();
        data.write_all(&payload).unwrap();
        let data: ZBuf = data.into();
        let routing_context = None;
        let reply_context = None;
        let attachment = None;

        let message = ZenohMessage::make_data(
            key.into(),
            data,
            channel,
            congestion_control,
            info,
            routing_context,
            reply_context,
            attachment,
        );

        // Insert the pending ping
        let barrier = Arc::new(Barrier::new(2));
        pending.lock().unwrap().insert(count, barrier.clone());
        let now = Instant::now();
        session.handle_message(message).unwrap();
        // Wait for the pong to arrive
        barrier.wait();
        println!(
            "session,{},latency.sequential,{},{},{},{},{}",
            opt.scenario,
            opt.name,
            payload.len(),
            opt.interval,
            count,
            now.elapsed().as_micros()
        );

        task::sleep(sleep).await;
        count += 1;
    }
}

async fn parallel(opt: Opt, whatami: WhatAmI) {
    let pending: Arc<Mutex<HashMap<u64, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let manager = TransportManager::builder()
        .whatami(whatami)
        .build(Arc::new(MySHParallel::new(
            opt.scenario,
            opt.name,
            opt.interval,
            pending.clone(),
        )))
        .unwrap();

    // Connect to publisher
    let session = manager
        .open_transport(EndPoint::from_str(opt.endpoint.as_str()).unwrap())
        .await
        .unwrap();

    let sleep = Duration::from_secs_f64(opt.interval);
    let payload = vec![0u8; opt.payload - 8];
    let mut count: u64 = 0;
    loop {
        // Create and send the message
        let channel = Channel {
            priority: Priority::Data,
            reliability: Reliability::Reliable,
        };
        let congestion_control = CongestionControl::Block;
        let key = "/test/ping";
        let info = None;

        let mut data: WBuf = WBuf::new(opt.payload, true);
        let count_bytes: [u8; 8] = count.to_le_bytes();
        data.write_all(&count_bytes).unwrap();
        data.write_all(&payload).unwrap();
        let data: ZBuf = data.into();
        let routing_context = None;
        let reply_context = None;
        let attachment = None;

        let message = ZenohMessage::make_data(
            key.into(),
            data,
            channel,
            congestion_control,
            info,
            routing_context,
            reply_context,
            attachment,
        );

        // Insert the pending ping
        pending.lock().unwrap().insert(count, Instant::now());

        session.handle_message(message).unwrap();

        task::sleep(sleep).await;
        count += 1;
    }
}

#[async_std::main]
async fn main() {
    // Enable logging
    env_logger::init();

    // Parse the args
    let opt = Opt::parse();

    let whatami = WhatAmI::from_str(opt.mode.as_str()).unwrap();

    if opt.parallel {
        parallel(opt, whatami).await;
    } else {
        single(opt, whatami).await;
    }
}
