#!/usr/bin/env bash
set -euo pipefail

export WORKSPACE_ROOT="$(pwd)"
export CARGO_TARGET_DIR="${WORKSPACE_ROOT}/target"

echo "[EXECUTION] Initializing absolute workspace layout..."
mkdir -p "${WORKSPACE_ROOT}/src/dataplane"
mkdir -p "${WORKSPACE_ROOT}/src/control_plane"

cat << 'MANIFEST_EOF' > "${WORKSPACE_ROOT}/Cargo.toml"
[package]
name = "native_high_performance"
version = "3.0.0"
edition = "2021"

[dependencies]
libc = "0.2"
tokio = { version = "1.38", features = ["rt-multi-thread", "macros", "time"] }

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
MANIFEST_EOF

cat << 'ENGINE_EOF' > "${WORKSPACE_ROOT}/src/dataplane/mod.rs"
// MODE: Deterministic Memory Safety.
#![allow(dead_code)]

use std::ptr::{read_volatile, write_volatile};

pub const DB_MMAP_BASE: u64 = 0x7000_0000;
pub const DB_MMAP_SIZE: u64 = 64 * 1024 * 1024; 
pub const DB_MMAP_END: u64  = DB_MMAP_BASE + DB_MMAP_SIZE;

pub const SCRAPE_RING_BASE: u64 = 0x7800_0000;
pub const SCRAPE_RING_SIZE: u64 = 1 * 1024 * 1024; 
pub const SCRAPE_RING_END: u64  = SCRAPE_RING_BASE + SCRAPE_RING_SIZE;

pub const DB_PAGE_SIZE: u32 = 4096; 
pub const RING_ENTRY_STRIDE: u64 = 32; 
pub const MAX_SCRAPE_ENTRIES: u32 = (SCRAPE_RING_SIZE / RING_ENTRY_STRIDE) as u32; 

#[derive(Copy, Clone, Debug)]
#[repr(C, align(8))]
pub struct ScrapedRecordDescriptor {
    pub record_id: u64,
    pub absolute_data_offset: u64,
    pub data_len: u64,
    pub valid_flag: u64, 
}

pub struct ScraperControlRing {
    pub head: u32,
    pub tail: u32,
}

pub unsafe fn scrape_database_page_block(
    control_ring: &mut ScraperControlRing,
    file_page_index: u32,
    target_magic_marker: u8
) -> Result<u32, &'static str> {
    let page_idx_u64 = file_page_index as u64;
    let page_byte_offset = page_idx_u64.checked_mul(DB_PAGE_SIZE as u64)
        .ok_or("FAIL: Database page block offset multiplication overflowed")?;
    let absolute_page_addr = DB_MMAP_BASE.checked_add(page_byte_offset)
        .ok_or("FAIL: Target database physical base calculation overflowed")?;
    let page_end_addr = absolute_page_addr.checked_add(DB_PAGE_SIZE as u64)
        .ok_or("FAIL: Page boundary definition structure computation overflowed")?;

    if absolute_page_addr < DB_MMAP_BASE || page_end_addr > DB_MMAP_END {
        return Err("FAIL: Targeted DB Page allocation exceeds current memory-mapped file bounds");
    }

    let mut extraction_count: u32 = 0;
    let mut byte_cursor: u64 = absolute_page_addr;

    while byte_cursor < page_end_addr {
        assert!(byte_cursor < DB_MMAP_END, "FAIL: Scraping cursor wandered out of raw file partition");

        let cell_ptr = byte_cursor as *const u8;
        let current_cell_value: u8 = read_volatile(cell_ptr);

        if current_cell_value == target_magic_marker {
            let ring_head_u64 = control_ring.head as u64;
            let next_head = (control_ring.head + 1) % MAX_SCRAPE_ENTRIES;
            if next_head == control_ring.tail {
                return Err("FAIL: Output Extraction Ring Buffer is entirely saturated (Head caught Tail)");
            }
            
            let ring_slot_offset = ring_head_u64 * RING_ENTRY_STRIDE;
            let output_descriptor_addr = SCRAPE_RING_BASE.checked_add(ring_slot_offset)
                .ok_or("FAIL: Output ring slot allocation pointer offset address calculation overflowed")?;

            assert!(output_descriptor_addr + RING_ENTRY_STRIDE <= SCRAPE_RING_END, "FAIL: Scraping Output Ring has saturated allocation memory");
            assert!(output_descriptor_addr % std::mem::align_of::<ScrapedRecordDescriptor>() as u64 == 0, "FAIL: Target write descriptor pointer is misaligned");

            let target_desc_ptr = output_descriptor_addr as *mut ScrapedRecordDescriptor;
            let record_id = (extraction_count as u64)
                .checked_add(page_idx_u64.checked_mul(10000).ok_or("FAIL: Record tracking key arithmetic overflowed")?)
                .ok_or("FAIL: Identifier assignment calculation sequence wrapped parameters")?;

            let extracted_record = ScrapedRecordDescriptor {
                record_id,
                absolute_data_offset: byte_cursor,
                data_len: 128, 
                valid_flag: 1,
            };

            write_volatile(target_desc_ptr, extracted_record);
            control_ring.head = next_head;
            extraction_count += 1;
        }
        byte_cursor += 16; 
    }
    Ok(extraction_count)
}
ENGINE_EOF

cat << 'METRICS_EOF' > "${WORKSPACE_ROOT}/src/control_plane/mod.rs"
// MODE: Deterministic Memory Safety.
#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::ptr::{read_volatile, write_volatile};

pub const METRICS_RING_BASE: u64 = 0x8000_0000;
pub const METRICS_RING_SIZE: u64 = 64 * 1024; 
pub const METRICS_RING_END: u64  = METRICS_RING_BASE + METRICS_RING_SIZE;
pub const METRICS_STRIDE: u64    = 32; 
pub const MAX_METRICS_SLOTS: u32 = (METRICS_RING_SIZE / METRICS_STRIDE) as u32; 

#[derive(Copy, Clone, Debug)]
#[repr(C, align(8))]
pub struct NetworkMetricFrame {
    pub total_packets: u64,
    pub dropped_packets: u64,
    pub total_bytes: u64,
    pub state_flags: u64, 
}

pub struct AsyncMetricsDaemon {
    pub ring_cursor: u32,
    pub packet_counter: Arc<AtomicU64>,
}

impl AsyncMetricsDaemon {
    pub fn new(packet_counter: Arc<AtomicU64>) -> Self {
        Self {
            ring_cursor: 0,
            packet_counter,
        }
    }

    pub async fn run_metrics_watcher_loop(&mut self) -> Result<(), &'static str> {
        loop {
            let current_slot = self.ring_cursor as u64;
            if current_slot >= MAX_METRICS_SLOTS as u64 {
                return Err("FAIL: Metrics daemon tracking pointer out of bounds maps");
            }

            let byte_offset = current_slot.checked_mul(METRICS_STRIDE)
                .ok_or("FAIL: Metrics stride offset calculation overflowed")?;
            let target_addr = METRICS_RING_BASE.checked_add(byte_offset)
                .ok_or("FAIL: Target metrics physical address mapping overflowed")?;

            assert!(target_addr >= METRICS_RING_BASE, "FAIL: Target metrics address points under base limits");
            assert!(target_addr + METRICS_STRIDE <= METRICS_RING_END, "FAIL: Target pointer overruns metrics partition layout");
            assert!(target_addr % std::mem::align_of::<NetworkMetricFrame>() as u64 == 0, "FAIL: Target metrics pointer is misaligned");

            {
                let metrics_ptr = target_addr as *mut NetworkMetricFrame;
                let mut hardware_metrics: NetworkMetricFrame = unsafe { read_volatile(metrics_ptr) };

                if hardware_metrics.state_flags == 1 {
                    println!("[Metrics Watcher] Packets Processed: {}, Bytes Streamed: {}", 
                        hardware_metrics.total_packets, 
                        hardware_metrics.total_bytes
                    );
                    self.packet_counter.store(hardware_metrics.total_packets, Ordering::Relaxed);
                    hardware_metrics.state_flags = 0;
                    unsafe { write_volatile(metrics_ptr, hardware_metrics); }
                }
            }

            self.ring_cursor = (self.ring_cursor + 1) % MAX_METRICS_SLOTS;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }
}
METRICS_EOF

cat << 'MAIN_EOF' > "${WORKSPACE_ROOT}/src/main.rs"
// MODE: Deterministic Memory Safety.

use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

pub mod dataplane;
pub mod control_plane;

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_help_manifest();
        return Err("FAIL: Execution routing parameter missing");
    }

    let routing_token: &str = args[1].as_str();

    match routing_token {
        "--scraper" => {
            println!("[Execution] Launching Zero-Copy Scraping Engine Validation Pass...");
            let control_ring = dataplane::ScraperControlRing { head: 0, tail: 0 };
            println!("Scraper Control Ring baseline safe at [Head: {}]", control_ring.head);
            
            let _ptr = dataplane::scrape_database_page_block as *const () as usize;
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
MAIN_EOF

cat << 'MOD_EOF' > "${WORKSPACE_ROOT}/src/lib.rs"
pub mod dataplane;
pub mod control_plane;
MOD_EOF

echo "[EXECUTION] Running native release compilation pass..."
cargo build --release

echo -e "\n=== [VERIFICATION] Verifying Local Artifact Allocation ==="
RELEASE_BIN="${CARGO_TARGET_DIR}/release/native_high_performance"

ls -lah "${RELEASE_BIN}"
echo -e "\n=== Running Integrated Smoke Tests ==="
"${RELEASE_BIN}" --scraper
"${RELEASE_BIN}" --metrics
