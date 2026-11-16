// MODE: Deterministic Memory Safety.
mod http;
mod metrics;

use http::{run_http_server, StackBuf};
use metrics::AsyncMetricsDaemon;
use std::env;
use std::fmt::Write as FmtWrite;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let mut serve_mode = false;
    let mut bind_buf = StackBuf::<64>::new();
    let _ = bind_buf.write_str("127.0.0.1:9100");

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--serve" {
            serve_mode = true;
        } else if arg == "--bind" {
            if let Some(val) = args.next() {
                bind_buf.len = 0;
                let _ = bind_buf.write_str(&val);
            }
        }
    }
    let bind_addr = core::str::from_utf8(bind_buf.as_bytes()).unwrap_or("127.0.0.1:9100");

    println!("=== Initializing Tokio Native Async Metrics Engine ===");
    let shared_counter = Arc::new(AtomicU64::new(0));
    let mut daemon = AsyncMetricsDaemon::new(shared_counter.clone());
    println!("Async Daemon worker initialized safely onto multi-threaded runtime.");

    if serve_mode {
        tokio::select! {
            res = daemon.run_metrics_watcher_loop() => {
                if let Err(e) = res { eprintln!("[CRITICAL] Daemon loop aborted: {}", e); }
            }
            res = run_http_server(bind_addr, shared_counter.clone()) => {
                if let Err(e) = res { eprintln!("[CRITICAL] Telemetry HTTP server failed: {}", e); }
            }
        }
        return;
    }

    println!("Awaiting memory-mapped metric registers context loops...");
    tokio::select! {
        res = daemon.run_metrics_watcher_loop() => {
            if let Err(e) = res { eprintln!("[CRITICAL] Daemon loop aborted: {}", e); }
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
            println!("Engine verification bootstrap trace successfully ran for 2 seconds. Exiting cleanly.");
        }
    }
}
