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
use std::sync::{Arc, Mutex};
use zenoh::config::Config;
use zenoh::net::protocol::io::ZBuf;
use zenoh::net::protocol::proto::{DataInfo, RoutingContext};
use zenoh::net::routing::face::Face;
use zenoh::net::runtime::Runtime;
use zenoh::net::transport::Primitives;
use zenoh_protocol_core::{
    Channel, CongestionControl, ConsolidationStrategy, KeyExpr, PeerId, QueryTarget, QueryableInfo,
    Reliability, SubInfo, SubMode, WhatAmI, ZInt,
};

struct LatencyPrimitives {
    tx: Mutex<Option<Arc<Face>>>,
}

impl LatencyPrimitives {
    fn new() -> LatencyPrimitives {
        LatencyPrimitives {
            tx: Mutex::new(None),
        }
    }

    fn set_tx(&self, tx: Arc<Face>) {
        let mut guard = self.tx.lock().unwrap();
        *guard = Some(tx);
    }
}

impl Primitives for LatencyPrimitives {
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
        channel: Channel,
        congestion_control: CongestionControl,
        data_info: Option<DataInfo>,
        routing_context: Option<RoutingContext>,
    ) {
        let tx_primitive = self.tx.lock().unwrap();
        tx_primitive
            .as_ref()
            .unwrap()
            .decl_resource(1, &"/test/pong".into());
        let rid = KeyExpr::from(1);
        tx_primitive.as_ref().unwrap().send_data(
            &rid,
            payload,
            channel,
            congestion_control,
            data_info,
            routing_context,
        );
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
        _source_kind: ZInt,
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
#[clap(name = "r_pong")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
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

    let mut config = Config::default();

    config.scouting.multicast.set_enabled(Some(false)).unwrap();
    match opt.mode.as_str() {
        "peer" => {
            config.set_mode(Some(WhatAmI::Peer)).unwrap();
            config
                .listen
                .endpoints
                .extend(opt.endpoint.split(',').map(|e| e.parse().unwrap()));
        }
        "router" => {
            config.set_mode(Some(WhatAmI::Router)).unwrap();
            config
                .listen
                .endpoints
                .extend(opt.endpoint.split(',').map(|e| e.parse().unwrap()));
        }
        "client" => {
            config.set_mode(Some(WhatAmI::Client)).unwrap();
            config
                .connect
                .endpoints
                .extend(opt.endpoint.split(',').map(|e| e.parse().unwrap()));
        }
        _ => panic!("Unsupported mode: {}", opt.mode),
    };

    let runtime = Runtime::new(config).await.unwrap();
    let rx_primitives = Arc::new(LatencyPrimitives::new());
    let tx_primitives = runtime.router.new_primitives(rx_primitives.clone());
    rx_primitives.set_tx(tx_primitives.clone());

    tx_primitives.decl_resource(2, &"/test/ping".into());
    let rid = KeyExpr::from(2);
    let sub_info = SubInfo {
        reliability: Reliability::Reliable,
        mode: SubMode::Push,
        period: None,
    };
    tx_primitives.decl_subscriber(&rid, &sub_info, None);

    // Stop forever
    future::pending::<()>().await;
}
