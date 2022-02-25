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
use std::convert::TryInto;
use clap::Parser;
use zenoh::net::protocol::core::WhatAmI;

#[derive(Debug, Parser)]
#[clap(name = "z_pong")]
struct Opt {
    #[clap(short = "l", long = "locator")]
    locator: String,
    #[clap(short = "m", long = "mode")]
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
                .listeners
                .extend(opt.locator.split(',').map(|v| v.parse().unwrap()));
        }
        "client" => {
            config.set_mode(Some(WhatAmI::Client)).unwrap();
            config
                .peers
                .extend(opt.locator.split(',').map(|v| v.parse().unwrap()));
        }
        _ => panic!("Unsupported mode: {}", opt.mode),
    };

    config.scouting.multicast.set_enabled(Some(false)).unwrap();
    

    let zenoh = Zenoh::new(config.into()).await.unwrap();
    let workspace = zenoh.workspace(None).await.unwrap();
    let mut sub = workspace
        .subscribe(&"/test/ping/".to_string().try_into().unwrap())
        .await
        .unwrap();

    while let Some(change) = sub.next().await {
        match change.value.unwrap() {
            Value::Raw(_, payload) => {
                workspace
                    .put(&"/test/pong".try_into().unwrap(), payload.into())
                    .wait()
                    .unwrap();
            }
            _ => panic!("Invalid value"),
        }
    }

    // Stop forever
    future::pending::<()>().await;
}
