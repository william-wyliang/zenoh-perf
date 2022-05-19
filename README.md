# zenoh-perf
Rust code for testing and validating zenoh

**Note** that this tool is under its way to support zenoh 0.6.0. 
Currently, ready programs include:
* throughput
  * z_put_thr and z_sub_thr
  * t_pub_thr, t_sub_thr, t_pubsub_thr and t_router_thr
  * r_pub_thr and r_sub_thr
* latency
  * z_ping and z_pong
  * r_ping and r_pong
  * t_ping and t_pong
  * t_pub_delay and t_sub_delay

**Compilation** for the ready programs
```
cargo build --release \
  --bin z_put_thr --bin z_sub_thr \
  --bin t_pub_thr --bin t_sub_thr \
  --bin t_pubsub_thr --bin t_router_thr \
  --bin r_pub_thr --bin r_sub_thr \
  --bin z_ping --bin z_pong \
  --bin r_ping --bin r_pong \
  --bin t_ping --bin t_pong \
  --bin t_pub_delay --bin t_sub_delay
```

_Other noticeable things_:
* the locator option has been changed to --endpoint or -e
* new options are added for performance comparison purpose:
    * --use-expr: to use declare_expr() to declare the key expression
    * --declare-publication: to call declare_publication() before publication
    * --no-callback: to use receiver() for subscriber instead of using the callback (for z_sub_thr only)
