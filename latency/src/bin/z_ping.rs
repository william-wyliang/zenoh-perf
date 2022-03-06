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
use async_std::sync::{Arc, Barrier, Mutex};
use async_std::task;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use zenoh::config::Config;
use zenoh::net::protocol::core::WhatAmI;
use zenoh::net::protocol::io::reader::{HasReader, Reader};
use zenoh::net::protocol::io::SplitBuffer;
use zenoh::prelude::*;


#[derive(Debug, StructOpt)]
#[structopt(name = "z_ping")]
struct Opt {
    #[structopt(short = "l", long = "locator")]
    locator: Option<String>,
    #[structopt(short = "m", long = "mode")]
    mode: String,
    #[structopt(short = "p", long = "payload")]
    payload: usize,
    #[structopt(short = "n", long = "name")]
    name: String,
    #[structopt(short = "s", long = "scenario")]
    scenario: String,
    #[structopt(short = "i", long = "interval")]
    interval: f64,
    #[structopt(long = "parallel")]
    parallel: bool,
}

async fn parallel(opt: Opt, config: Config) {
    let session = zenoh::open(config).await.unwrap();
    let session = Arc::new(session);

    // The hashmap with the pings
    let pending = Arc::new(Mutex::new(HashMap::<u64, Instant>::new()));
    let barrier = Arc::new(Barrier::new(2));

    let c_pending = pending.clone();
    let c_barrier = barrier.clone();
    let c_session = session.clone();
    let scenario = opt.scenario;
    let name = opt.name;
    let interval = opt.interval;
    task::spawn(async move {
        let mut sub = c_session
            .subscribe("/test/pong/")
            .await
            .unwrap();

        // Notify that the subscriber has been created
        c_barrier.wait().await;

        while let Some(sample) = sub.next().await {
                  let mut payload_reader = sample.value.payload.reader();   
                  let mut count_bytes = [0u8; 8];
                  if payload_reader.read_exact(&mut count_bytes){
                    let count = u64::from_le_bytes(count_bytes);

                    let instant = c_pending.lock().await.remove(&count).unwrap();
                    println!(
                        "zenoh,{},latency.parallel,{},{},{},{},{}",
                        scenario,
                        name,
                        sample.value.payload.len(),
                        interval,
                        count,
                        instant.elapsed().as_micros()
                    );
                }  else {
                    panic!("Fail to fill the buffer");
                }
        }
        panic!("Invalid value!");
    });

    // Wait for the subscriber to be declared
    barrier.wait().await;

    let mut count: u64 = 0;
    loop {
        let count_bytes: [u8; 8] = count.to_le_bytes();
        let mut payload = vec![0u8; opt.payload];
        payload[0..8].copy_from_slice(&count_bytes);

        pending.lock().await.insert(count, Instant::now());

        session
            .put("/test/ping", payload)
            .await
            .unwrap();

        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}

async fn single(opt: Opt, config: Config) {
    let session = zenoh::open(config).await.unwrap();

    let scenario = opt.scenario;
    let name = opt.name;
    let interval = opt.interval;

    let mut sub = session
        .subscribe("/test/pong/")
        .await
        .unwrap();

    let mut count: u64 = 0;
    loop {
        let count_bytes: [u8; 8] = count.to_le_bytes();
        let mut payload = vec![0u8; opt.payload];
        payload[0..8].copy_from_slice(&count_bytes);

        let now = Instant::now();
        session
            .put("/test/ping", payload)
            .await
            .unwrap();

        match sub.next().await{
            Some(sample) => {
                let mut payload_reader = sample.value.payload.reader();
                let mut count_bytes = [0u8; 8];
                if payload_reader.read_exact(&mut count_bytes) {
                    let s_count = u64::from_le_bytes(count_bytes);

                println!(
                    "zenoh,{},latency.sequential,{},{},{},{},{}",
                    scenario,
                    name,
                    sample.value.payload.len(),
                    interval,
                    s_count,
                    now.elapsed().as_micros()
                );
            } else {
                panic!("Fail to fill the buffer");
            }
        }
           _ => panic!("Invalid value"),
        }
        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    // Parse the args
    let opt = Opt::from_args();

    let mut config = Config::default();
    match opt.mode.as_str() {
        "peer" => config.set_mode(Some(WhatAmI::Peer)).unwrap(),
        "client" => config.set_mode(Some(WhatAmI::Client)).unwrap(),
        _ => panic!("Unsupported mode: {}", opt.mode),
    };

    if let Some(ref l) = opt.locator {
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        config
            .peers
            .extend(l.split(',').map(|v| v.parse().unwrap()));
    } else {
        config.scouting.multicast.set_enabled(Some(true)).unwrap();
    }

    if opt.parallel {
        parallel(opt, config).await;
    } else {
        single(opt, config).await;
    }
}
