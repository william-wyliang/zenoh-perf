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
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use zenoh::{
    config::Config,
    net::{
        protocol::{
            io::ZBuf,
            proto::{DataInfo, RoutingContext},
        },
        runtime::Runtime,
        transport::Primitives,
    },
};
use zenoh_protocol_core::{
    Channel, CongestionControl, ConsolidationStrategy, EndPoint, KeyExpr, PeerId, QueryTarget,
    QueryableInfo, Reliability, SubInfo, SubMode, WhatAmI, ZInt,
};

struct ThroughputPrimitives {
    count: Arc<AtomicUsize>,
}

impl ThroughputPrimitives {
    pub fn new(count: Arc<AtomicUsize>) -> ThroughputPrimitives {
        ThroughputPrimitives { count }
    }
}

impl Primitives for ThroughputPrimitives {
    fn decl_resource(&self, _expr_id: ZInt, _key_expr: &KeyExpr) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn forget_resource(&self, _expr_id: ZInt) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn decl_publisher(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn forget_publisher(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn decl_subscriber(
        &self,
        _key_expr: &KeyExpr,
        _sub_info: &SubInfo,
        _routing_context: Option<RoutingContext>,
    ) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn forget_subscriber(&self, _key_expr: &KeyExpr, _routing_context: Option<RoutingContext>) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn decl_queryable(
        &self,
        _key_expr: &KeyExpr,
        _kind: ZInt,
        _qable_info: &QueryableInfo,
        _routing_context: Option<RoutingContext>,
    ) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn forget_queryable(
        &self,
        _key_expr: &KeyExpr,
        _kind: ZInt,
        _routing_context: Option<RoutingContext>,
    ) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn send_data(
        &self,
        _key_expr: &KeyExpr,
        _payload: ZBuf,
        _channel: Channel,
        _congestion_control: CongestionControl,
        _data_info: Option<DataInfo>,
        _routing_context: Option<RoutingContext>,
    ) {
        self.count.fetch_add(1, Ordering::Relaxed);
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
        self.count.fetch_add(1, Ordering::Relaxed);
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
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn send_reply_final(&self, _qid: ZInt) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn send_pull(
        &self,
        _is_final: bool,
        _key_expr: &KeyExpr,
        _pull_id: ZInt,
        _max_samples: &Option<ZInt>,
    ) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn send_close(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Parser)]
#[clap(name = "r_sub_thr")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, value_delimiter = ',')]
    endpoint: Vec<EndPoint>,

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
        endpoint,
        mode,
        payload,
        name,
        scenario,
        config,
    } = Opt::parse();

    let config = {
        let mut config: Config = if let Some(path) = config {
            Config::from_file(path).unwrap()
        } else {
            Config::default()
        };
        config.set_mode(Some(mode)).unwrap();
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        match mode {
            WhatAmI::Peer | WhatAmI::Router => config.listen.endpoints.extend(endpoint),
            WhatAmI::Client => config.connect.endpoints.extend(endpoint),
        };
        config
    };

    let count = Arc::new(AtomicUsize::new(0));
    let my_primitives = Arc::new(ThroughputPrimitives::new(count.clone()));

    let runtime = Runtime::new(config).await.unwrap();
    let primitives = runtime.router.new_primitives(my_primitives);

    primitives.decl_resource(1, &"/test/thr".to_string().into());

    let rid = KeyExpr::from(1);
    let sub_info = SubInfo {
        reliability: Reliability::Reliable,
        mode: SubMode::Push,
        period: None,
    };
    primitives.decl_subscriber(&rid, &sub_info, None);

    loop {
        let now = Instant::now();
        task::sleep(Duration::from_secs(1)).await;
        let elapsed = now.elapsed().as_micros() as f64;

        let c = count.swap(0, Ordering::Relaxed);
        if c > 0 {
            let interval = 1_000_000.0 / elapsed;
            println!(
                "router,{},throughput,{},{},{}",
                scenario,
                name,
                payload,
                (c as f64 / interval).floor() as usize
            );
        }
    }
}
