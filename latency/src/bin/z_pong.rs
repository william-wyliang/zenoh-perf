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
use async_std::stream::StreamExt;
use clap::Parser;
use zenoh::config::Config;
use zenoh_protocol_core::{CongestionControl, WhatAmI};

#[derive(Debug, Parser)]
#[clap(name = "z_pong")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long)]
    endpoint: String,

    /// peer or client
    #[clap(short, long, possible_values = ["peer", "client"])]
    mode: String,

    /// declare a numerical ID for key expression
    #[clap(long)]
    use_expr: bool,

    /// declare publication before the publisher
    #[clap(long)]
    declare_publication: bool,
}

const KEY_EXPR_PING: &str = "/test/ping";
const KEY_EXPR_PONG: &str = "/test/pong";

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    // Parse the args
    let opt = Opt::parse();

    let mut config = Config::default();
    match opt.mode.as_str() {
        "peer" => {
            config.set_mode(Some(WhatAmI::Peer)).unwrap();
            config
                .listen
                .endpoints
                .extend(opt.endpoint.split(',').map(|v| v.parse().unwrap()));
        }
        "client" => {
            config.set_mode(Some(WhatAmI::Client)).unwrap();
            config
                .connect
                .endpoints
                .extend(opt.endpoint.split(',').map(|v| v.parse().unwrap()));
        }
        _ => panic!("Unsupported mode: {}", opt.mode),
    };
    config.scouting.multicast.set_enabled(Some(false)).unwrap();

    let session = zenoh::open(config).await.unwrap();
    let mut sub = if opt.use_expr {
        // Declare the subscriber
        let key_expr_ping = session.declare_expr(KEY_EXPR_PING).await.unwrap();
        session.subscribe(key_expr_ping).reliable().await.unwrap()
    } else {
        session.subscribe(KEY_EXPR_PING).reliable().await.unwrap()
    };
    let mut key_expr_pong = 0;
    if opt.use_expr {
        key_expr_pong = session.declare_expr(KEY_EXPR_PONG).await.unwrap();
        if opt.declare_publication {
            session.declare_publication(key_expr_pong).await.unwrap();
        }
    } else if opt.declare_publication {
        session.declare_publication(KEY_EXPR_PONG).await.unwrap();
    }

    while let Some(sample) = sub.next().await {
        let writer = if opt.use_expr {
            session.put(key_expr_pong, sample)
        } else {
            session.put(KEY_EXPR_PONG, sample)
        };
        writer
            .congestion_control(CongestionControl::Block)
            .await
            .unwrap();
    }

    // Stop forever
    future::pending::<()>().await;
}
