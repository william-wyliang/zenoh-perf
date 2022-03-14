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
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use zenoh::{
    config::{whatami::WhatAmI, Config},
    prelude::*,
};

#[derive(Debug, Parser)]
#[clap(name = "zn_sub_thr_stream")]
struct Opt {
    /// locator(s), e.g. --locator tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, value_delimiter = ',')]
    locator: Vec<Locator>,

    /// peer, router, or client
    #[clap(short, long)]
    mode: String,

    /// payload size (bytes)
    #[clap(short, long)]
    payload: usize,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    scenario: String,

    /// configuration file (json5 or yaml)
    #[clap(long = "conf")]
    config: Option<PathBuf>,
}

const KEY_EXPR: &str = "/test/thr";

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    // Parse the args
    let Opt {
        locator,
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
        let mode = WhatAmI::from_str(&mode).unwrap();
        config.set_mode(Some(mode)).unwrap();
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        match mode {
            WhatAmI::Peer => config.set_listeners(locator).unwrap(),
            WhatAmI::Client => config.set_peers(locator).unwrap(),
            _ => panic!("Unsupported mode: {}", mode),
        };
        config
    };

    let session = zenoh::open(config).await.unwrap();
    let expr_id = session.declare_expr(KEY_EXPR).await.unwrap();

    let messages = Arc::new(AtomicUsize::new(0));
    let c_messages = messages.clone();

    task::spawn(async move {
        loop {
            let now = Instant::now();
            task::sleep(Duration::from_secs(1)).await;
            let elapsed = now.elapsed().as_micros() as f64;

            let c = c_messages.swap(0, Ordering::Relaxed);
            if c > 0 {
                let interval = 1_000_000.0 / elapsed;
                println!(
                    "zenoh-net,{},throughput,{},{},{}",
                    scenario,
                    name,
                    payload,
                    (c as f64 / interval).floor() as usize
                );
            }
        }
    });

    let mut sub = session
        .subscribe(expr_id)
        .reliable()
        .push_mode()
        .await
        .unwrap();

    while sub.receiver().recv().is_ok() {
        messages.fetch_add(1, Ordering::Relaxed);
    }
}
