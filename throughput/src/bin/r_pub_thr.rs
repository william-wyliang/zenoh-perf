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
    time::Duration,
};
use zenoh::{
    config::Config,
    net::{
        protocol::io::ZBuf,
        runtime::Runtime,
        transport::{DummyPrimitives, Primitives},
    },
};
use zenoh_protocol_core::{
    Channel, CongestionControl, EndPoint, KeyExpr, Priority, Reliability, WhatAmI,
};

#[derive(Debug, Parser)]
#[clap(name = "r_pub_thr")]
struct Opt {
    /// endpoint(s), e.g. --endpoint tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long, required(true), value_delimiter = ',')]
    endpoint: Vec<EndPoint>,

    /// peer, router, or client
    #[clap(short, long)]
    mode: WhatAmI,

    /// payload size (bytes)
    #[clap(short, long)]
    payload: usize,

    /// print the counter
    #[clap(short = 't', long)]
    print: bool,

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
        print,
        config,
    } = Opt::parse();

    let config = {
        let mut config: Config = if let Some(path) = config {
            Config::from_file(path).unwrap()
        } else {
            Config::default()
        };
        config.set_mode(Some(mode)).unwrap();
        config.set_add_timestamp(Some(false)).unwrap();
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        config.connect.endpoints.extend(endpoint);
        config
    };

    let my_primitives = Arc::new(DummyPrimitives::new());

    let runtime = Runtime::new(config).await.unwrap();
    let primitives = runtime.router.new_primitives(my_primitives);

    primitives.decl_resource(1, &"/test/thr".to_string().into());
    let rid = KeyExpr::from(1);
    primitives.decl_publisher(&rid, None);

    // @TODO: Fix writer starvation in the RwLock and remove this sleep
    // Wait for the declare to arrive
    task::sleep(Duration::from_millis(1_000)).await;

    let channel = Channel {
        priority: Priority::Data,
        reliability: Reliability::Reliable,
    };
    let congestion_control = CongestionControl::Block;
    let payload = ZBuf::from(vec![0u8; payload]);
    if print {
        let count = Arc::new(AtomicUsize::new(0));
        let c_count = count.clone();
        task::spawn(async move {
            loop {
                task::sleep(Duration::from_secs(1)).await;
                let c = count.swap(0, Ordering::Relaxed);
                if c > 0 {
                    println!("{} msg/s", c);
                }
            }
        });

        loop {
            primitives.send_data(
                &rid,
                payload.clone(),
                channel,
                congestion_control,
                None,
                None,
            );
            c_count.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        loop {
            primitives.send_data(
                &rid,
                payload.clone(),
                channel,
                congestion_control,
                None,
                None,
            );
        }
    }
}
