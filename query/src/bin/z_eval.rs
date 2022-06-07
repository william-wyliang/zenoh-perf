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
use zenoh::{config::Config, prelude::Sample, queryable::EVAL};
use zenoh_protocol_core::{EndPoint, WhatAmI};

#[derive(Debug, Parser)]
#[clap(name = "z_eval")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long)]
    endpoint: Vec<EndPoint>,

    #[clap(short, long)]
    mode: WhatAmI,

    #[clap(short, long)]
    payload: usize,
}

const KEY_EXPR: &str = "/test/query";

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    // Parse the args
    let Opt {
        endpoint,
        mode,
        payload,
    } = Opt::parse();
    let config = {
        let mut config: Config = Config::default();
        config.set_mode(Some(mode)).unwrap();
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        match mode {
            WhatAmI::Peer => config.listen.endpoints.extend(endpoint),
            WhatAmI::Client => config.connect.endpoints.extend(endpoint),
            _ => panic!("Unsupported mode: {}", mode),
        }
        config
    };

    let session = zenoh::open(config).await.unwrap();
    let mut queryable = session.queryable(KEY_EXPR).kind(EVAL).await.unwrap();
    while let Some(query) = queryable.next().await {
        query
            .reply_async(Sample::new(KEY_EXPR, vec![0u8; payload]))
            .await;
    }
}
