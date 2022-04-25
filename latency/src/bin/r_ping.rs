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
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;
use std::time::Instant;
use zenoh::config::Config;
use zenoh::net::protocol::io::reader::{HasReader, Reader};
use zenoh::net::protocol::io::SplitBuffer;
use zenoh::net::protocol::io::{WBuf, ZBuf};
use zenoh::net::protocol::proto::{DataInfo, RoutingContext};
use zenoh::net::runtime::Runtime;
use zenoh::net::transport::Primitives;
use zenoh_protocol_core::{
    Channel, CongestionControl, ConsolidationStrategy, KeyExpr, PeerId, Priority, QueryTarget,
    QueryableInfo, Reliability, SubInfo, SubMode, WhatAmI, ZInt,
};

const KEY_EXPR_PING: &str = "/test/ping";
const KEY_EXPR_PONG: &str = "/test/pong";

// Primitives for the non-blocking locator
struct LatencyPrimitivesParallel {
    scenario: String,
    name: String,
    interval: f64,
    pending: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl LatencyPrimitivesParallel {
    pub fn new(
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

impl Primitives for LatencyPrimitivesParallel {
    fn decl_resource(&self, _expr_id: ZInt, _key_expr: &KeyExpr) {}
    fn forget_resource(&self, _expr_id: ZInt) {}
    fn decl_publisher(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {}
    fn forget_publisher(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {}
    fn decl_subscriber(
        &self,
        _key_expr: &KeyExpr,
        _sub_info: &SubInfo,
        _routing_context: Option<RoutingContext>,
    ) {
    }
    fn forget_subscriber(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {}
    fn decl_queryable(
        &self,
        _key_expr: &KeyExpr,
        _kind: ZInt,
        _qabl_info: &QueryableInfo,
        _routing_context: Option<RoutingContext>,
    ) {
    }
    fn forget_queryable(
        &self,
        _key_expr: &KeyExpr,
        _kind: ZInt,
        _routing_context: Option<RoutingContext>,
    ) {
    }

    fn send_data(
        &self,
        _key_expr: &KeyExpr,
        payload: ZBuf,
        _channel: Channel,
        _congestion_control: CongestionControl,
        _data_info: Option<DataInfo>,
        _routing_context: Option<RoutingContext>,
    ) {
        let mut count_bytes = [0u8; 8];
        let mut data_reader = payload.reader();
        if data_reader.read_exact(&mut count_bytes) {
            let count = u64::from_le_bytes(count_bytes);
            let instant = self.pending.lock().unwrap().remove(&count).unwrap();
            println!(
                "router,{},latency.parallel,{},{},{},{},{}",
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

    fn send_query(
        &self,
        _key_expr: &KeyExpr,
        _value_selector: &str,
        _qid: ZInt,
        _target: QueryTarget,
        _consolidation: ConsolidationStrategy,
        _routing_context: Option<RoutingContext>,
    ) {
    }
    fn send_reply_data(
        &self,
        _qid: ZInt,
        _replier_kind: ZInt,
        _replier_id: PeerId,
        _key_expr: KeyExpr,
        _info: Option<DataInfo>,
        _payload: ZBuf,
    ) {
    }
    fn send_reply_final(&self, _qid: ZInt) {}
    fn send_pull(
        &self,
        _is_final: bool,
        _key_expr: &KeyExpr,
        _pull_id: ZInt,
        _max_samples: &Option<ZInt>,
    ) {
    }
    fn send_close(&self) {}
}

// Primitives for the blocking locator
struct LatencyPrimitivesSequential {
    pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>>,
}

impl LatencyPrimitivesSequential {
    pub fn new(pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>>) -> Self {
        Self { pending }
    }
}

impl Primitives for LatencyPrimitivesSequential {
    fn decl_resource(&self, _expr_id: ZInt, _key_expr: &KeyExpr) {}
    fn forget_resource(&self, _expr_id: ZInt) {}
    fn decl_publisher(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {}
    fn forget_publisher(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {}
    fn decl_subscriber(
        &self,
        _key_expr: &KeyExpr,
        _sub_info: &SubInfo,
        _routing_context: Option<RoutingContext>,
    ) {
    }
    fn forget_subscriber(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {}
    fn decl_queryable(
        &self,
        _key_expr: &KeyExpr,
        _kind: ZInt,
        _qabl_info: &QueryableInfo,
        _routing_context: Option<RoutingContext>,
    ) {
    }
    fn forget_queryable(
        &self,
        _key_expr: &KeyExpr,
        _kind: ZInt,
        _routing_context: Option<RoutingContext>,
    ) {
    }

    fn send_data(
        &self,
        _key_expr: &KeyExpr,
        payload: ZBuf,
        _channel: Channel,
        _congestion_control: CongestionControl,
        _data_info: Option<DataInfo>,
        _routing_context: Option<RoutingContext>,
    ) {
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

    fn send_query(
        &self,
        _key_expr: &KeyExpr,
        _value_selector: &str,
        _qid: ZInt,
        _target: QueryTarget,
        _consolidation: ConsolidationStrategy,
        _routing_context: Option<RoutingContext>,
    ) {
    }
    fn send_reply_data(
        &self,
        _qid: ZInt,
        _replier_kind: ZInt,
        _replier_id: PeerId,
        _key_expr: KeyExpr,
        _info: Option<DataInfo>,
        _payload: ZBuf,
    ) {
    }
    fn send_reply_final(&self, _qid: ZInt) {}
    fn send_pull(
        &self,
        _is_final: bool,
        _key_expr: &KeyExpr,
        _pull_id: ZInt,
        _max_samples: &Option<ZInt>,
    ) {
    }
    fn send_close(&self) {}
}

#[derive(Debug, Parser)]
#[clap(name = "r_ping")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
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
async fn parallel(opt: Opt, config: Config) {
    let pending: Arc<Mutex<HashMap<u64, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

    let runtime = Runtime::new(config).await.unwrap();
    let rx_primitives = Arc::new(LatencyPrimitivesParallel::new(
        opt.scenario,
        opt.name,
        opt.interval,
        pending.clone(),
    ));
    let tx_primitives = runtime.router.new_primitives(rx_primitives);

    tx_primitives.decl_resource(1, &KEY_EXPR_PONG.into());
    let rid = KeyExpr::from(1);
    let sub_info = SubInfo {
        reliability: Reliability::Reliable,
        mode: SubMode::Push,
        period: None,
    };
    tx_primitives.decl_subscriber(&rid, &sub_info, None);

    let channel = Channel {
        priority: Priority::Data,
        reliability: Reliability::Reliable,
    };
    let congestion_control = CongestionControl::Block;
    let payload = vec![0u8; opt.payload - 8];
    let mut count: u64 = 0;

    tx_primitives.decl_resource(2, &KEY_EXPR_PING.into());
    let rid = KeyExpr::from(2);

    loop {
        // Create and send the message
        let mut data: WBuf = WBuf::new(opt.payload, true);
        let count_bytes: [u8; 8] = count.to_le_bytes();
        data.write_all(&count_bytes).unwrap();
        data.write_all(&payload).unwrap();
        let data: ZBuf = data.into();

        // Insert the pending ping
        pending.lock().unwrap().insert(count, Instant::now());

        tx_primitives.send_data(&rid, data, channel, congestion_control, None, None);

        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}

async fn single(opt: Opt, config: Config) {
    let pending: Arc<Mutex<HashMap<u64, Arc<Barrier>>>> = Arc::new(Mutex::new(HashMap::new()));

    let runtime = Runtime::new(config).await.unwrap();
    let rx_primitives = Arc::new(LatencyPrimitivesSequential::new(pending.clone()));
    let tx_primitives = runtime.router.new_primitives(rx_primitives);

    tx_primitives.decl_resource(1, &KEY_EXPR_PONG.into());
    let rid = KeyExpr::from(1);
    let sub_info = SubInfo {
        reliability: Reliability::Reliable,
        mode: SubMode::Push,
        period: None,
    };
    tx_primitives.decl_subscriber(&rid, &sub_info, None);

    let channel = Channel {
        priority: Priority::Data,
        reliability: Reliability::Reliable,
    };
    let congestion_control = CongestionControl::Block;
    let payload = vec![0u8; opt.payload - 8];
    let mut count: u64 = 0;
    tx_primitives.decl_resource(2, &KEY_EXPR_PING.into());
    let rid = KeyExpr::from(2);
    loop {
        // Create and send the message
        let mut data: WBuf = WBuf::new(opt.payload, true);
        let count_bytes: [u8; 8] = count.to_le_bytes();
        data.write_all(&count_bytes).unwrap();
        data.write_all(&payload).unwrap();
        let data: ZBuf = data.into();

        // Insert the pending ping
        let barrier = Arc::new(Barrier::new(2));
        pending.lock().unwrap().insert(count, barrier.clone());

        let now = Instant::now();
        tx_primitives.send_data(&rid, data, channel, congestion_control, None, None);
        barrier.wait();
        println!(
            "router,{},latency.sequential,{},{},{},{},{}",
            opt.scenario,
            opt.name,
            payload.len(),
            opt.interval,
            count,
            now.elapsed().as_micros()
        );

        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}

#[async_std::main]
async fn main() {
    // Enable logging
    env_logger::init();

    // Parse the args
    let opt = Opt::parse();

    let mut config = Config::default();
    match opt.mode.as_str() {
        "peer" => config.set_mode(Some(WhatAmI::Peer)).unwrap(),
        "client" => config.set_mode(Some(WhatAmI::Client)).unwrap(),
        "router" => config.set_mode(Some(WhatAmI::Router)).unwrap(),
        _ => panic!("Unsupported mode {}", opt.mode),
    };
    config.scouting.multicast.set_enabled(Some(false)).unwrap();
    config
        .connect
        .endpoints
        .extend(opt.endpoint.split(',').map(|e| e.parse().unwrap()));

    if opt.parallel {
        parallel(opt, config).await;
    } else {
        single(opt, config).await;
    }
}
