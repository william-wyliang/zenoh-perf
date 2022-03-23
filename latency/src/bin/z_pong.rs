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
}

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
    let mut sub = session
        .subscribe("/test/ping/")
        .reliable()
        .await
        .unwrap();

    while let Some(sample) = sub.next().await {
                session
                    .put("/test/pong", sample)
                    .congestion_control(CongestionControl::Block)
                    .await
                    .unwrap();
    }

    // Stop forever
    future::pending::<()>().await;
}
