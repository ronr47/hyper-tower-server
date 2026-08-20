// MODE: Deterministic Memory Safety.
mod metrics;

use metrics::AsyncMetricsDaemon;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[tokio::main]
async fn main() {
    println!("=== Initializing Tokio Native Async Metrics Engine ===");

    let shared_counter = Arc::new(AtomicU64::new(0));
    let mut daemon = AsyncMetricsDaemon::new(shared_counter.clone());

    println!("Async Daemon worker initialized safely onto multi-threaded runtime.");
    println!("Awaiting memory-mapped metric registers context loops...");

    // Basic timeout handle wrapper to show execution mechanics work cleanly
    let daemon_future = daemon.run_metrics_watcher_loop();
    
    tokio::select! {
        res = daemon_future => {
            if let Err(e) = res {
                eprintln!("[CRITICAL] Daemon loop aborted with error: {}", e);
            }
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
            println!("Engine verification bootstrap trace successfully ran for 2 seconds. Exiting cleanly.");
        }
    }
}
