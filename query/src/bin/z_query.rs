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
use async_std::stream::StreamExt;
use clap::Parser;
use std::time::Instant;
use zenoh::config::Config;
use zenoh_buffers::SplitBuffer;
use zenoh_protocol_core::{EndPoint, WhatAmI};

#[derive(Debug, Parser)]
#[clap(name = "z_query")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long)]
    endpoint: Vec<EndPoint>,

    /// peer, router, or client
    #[clap(short, long)]
    mode: WhatAmI,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    scenario: String,
}

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    // Parse the args
    let Opt {
        endpoint,
        mode,
        name,
        scenario,
    } = Opt::parse();
    let config = {
        let mut config: Config = Config::default();
        config.set_mode(Some(mode)).unwrap();
        config.set_add_timestamp(Some(false)).unwrap();
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        config.connect.endpoints.extend(endpoint);
        config
    };

    let session = zenoh::open(config).await.unwrap();

    let mut count: u64 = 0;
    loop {
        let now = Instant::now();
        let mut data_stream = session.get("/test/query").await.unwrap();

        let mut payload: usize = 0;
        while let Some(reply) = data_stream.next().await {
            payload += reply.sample.value.payload.len();
        }

        println!(
            "zenoh,{},query.latency,{},{},{},{}",
            scenario,
            name,
            payload,
            count,
            now.elapsed().as_micros()
        );
        count += 1;
    }
}
