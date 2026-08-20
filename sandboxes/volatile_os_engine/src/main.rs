use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

struct EngineState {
    bytes_written: AtomicU64,
}

fn fetch_disk_sectors(device_name: &str) -> u64 {
    if let Ok(file) = File::open("/proc/diskstats") {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 7 && parts[2] == device_name {
                if let Ok(sectors) = parts[9].parse::<u64>() {
                    return sectors * 512;
                }
            }
        }
    }
    0
}

fn spawn_telemetry_worker(state: Arc<EngineState>, target_device: String) {
    std::thread::spawn(move || {
        let mut last_bytes = fetch_disk_sectors(&target_device);
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let current_bytes = fetch_disk_sectors(&target_device);
            let delta = if current_bytes >= last_bytes {
                current_bytes - last_bytes
            } else {
                0
            };
            state.bytes_written.store(delta, Ordering::Relaxed);
            last_bytes = current_bytes;
        }
    });
}

async fn handle_client(mut stream: TcpStream, state: Arc<EngineState>, start_time: Instant) {
    let actual_written = state.bytes_written.load(Ordering::Relaxed);
    let duration = start_time.elapsed();

    let response_body = format!(
        "{{\n  \"engine_status\": \"HYPERTOWER_ACTIVE\",\n  \"telemetry\": {{\n    \"bytes_written_delta_100ms\": {},\n    \"latency_us\": {}\n  }}\n}}",
        actual_written,
        duration.as_micros()
    );

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );

    let _ = stream.write_all(response.as_bytes()).await;
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    println!("==========================================================");
    println!("🏰 HYPER-TOWER ONLINE: DEPLOYING ASYNC NETWORK PIPELINE");
    println!("==========================================================");

    let target_device = std::env::var("TARGET_DISK").unwrap_or_else(|_| "sda".to_string());
    let state = Arc::new(EngineState {
        bytes_written: AtomicU64::new(0),
    });

    spawn_telemetry_worker(Arc::clone(&state), target_device.clone());
    
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    println!("Hash-Signal Pipeline exposed for disk [{}] on port 8080...", target_device);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state_ref = Arc::clone(&state);
                tokio::spawn(async move {
                    let start_time = Instant::now();
                    handle_client(stream, state_ref, start_time).await;
                });
            }
            Err(e) => eprintln!("⚠️ Connection accept error: {}", e),
        }
    }
}
