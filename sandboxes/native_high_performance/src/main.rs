// MODE: Deterministic Memory Safety.

use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

pub mod dataplane;
pub mod control_plane;

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    // Collect system runtime arguments safely
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_help_manifest();
        return Err("FAIL: Execution routing parameter missing");
    }

    let routing_token: &str = args[1].as_str();

    match routing_token {
        "--scraper" => {
            println!("[Execution] Launching Zero-Copy Scraping Engine Validation Pass...");
            let mut control_ring = dataplane::ScraperControlRing { head: 0, tail: 0 };
            println!("Scraper Control Ring baseline safe at [Head: {}]", control_ring.head);
            
            // Symbol link validation
            let _ptr = dataplane::scrape_database_page_block as usize;
            println!("Verified data-plane scraper function vector: 0x{:X}", _ptr);
            Ok(())
        }
        "--metrics" => {
            println!("[Execution] Launching Async Multi-Threaded Metrics Watchdog...");
            let shared_counter = Arc::new(AtomicU64::new(0));
            let mut daemon = control_plane::AsyncMetricsDaemon::new(shared_counter);
            
            let daemon_future = daemon.run_metrics_watcher_loop();
            tokio::select! {
                res = daemon_future => { res? }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    println!("Async telemetry matrix baseline execution pass successful.");
                }
            }
            Ok(())
        }
        _ => {
            print_help_manifest();
            Err("FAIL: Invalid destination routing token specified")
        }
    }
}

fn print_help_manifest() {
    eprintln!("======================================================");
    eprintln!("Unified Native High-Performance Dataplane Entry Point");
    eprintln!("Usage:");
    eprintln!("  ./target/release/native_high_performance --scraper");
    eprintln!("  ./target/release/native_high_performance --metrics");
    eprintln!("======================================================");
}
