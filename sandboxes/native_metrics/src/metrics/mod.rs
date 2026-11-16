// MODE: Deterministic Memory Safety.
#![allow(dead_code)]

use std::ptr::{self, read_volatile, write_volatile};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub const METRICS_RING_BASE: u64 = 0x8000_0000;
pub const METRICS_RING_SIZE: u64 = 64 * 1024;
pub const METRICS_STRIDE: u64 = 32;
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
    pub ring_base: u64,
    pub ring_cursor: u32,
    pub packet_counter: Arc<AtomicU64>,
}

impl AsyncMetricsDaemon {
    pub fn new(packet_counter: Arc<AtomicU64>) -> Self {
        let mapped = unsafe {
            libc::mmap(
                METRICS_RING_BASE as *mut libc::c_void,
                METRICS_RING_SIZE as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        let ring_base = if mapped == libc::MAP_FAILED {
            let fallback = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    METRICS_RING_SIZE as usize,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            assert!(fallback != libc::MAP_FAILED, "FAIL: Unable to mmap metrics ring buffer");
            fallback as u64
        } else {
            mapped as u64
        };

        Self {
            ring_base,
            ring_cursor: 0,
            packet_counter,
        }
    }

    pub async fn run_metrics_watcher_loop(&mut self) -> Result<(), &'static str> {
        let ring_end = self
            .ring_base
            .checked_add(METRICS_RING_SIZE)
            .ok_or("FAIL: Ring end address overflowed")?;

        loop {
            let current_slot = self.ring_cursor as u64;

            if current_slot >= MAX_METRICS_SLOTS as u64 {
                return Err("FAIL: Metrics daemon tracking pointer out of bounds maps");
            }

            let byte_offset = current_slot
                .checked_mul(METRICS_STRIDE)
                .ok_or("FAIL: Metrics stride offset calculation overflowed")?;

            let target_addr = self
                .ring_base
                .checked_add(byte_offset)
                .ok_or("FAIL: Target metrics address mapping overflowed")?;

            assert!(target_addr >= self.ring_base, "FAIL: Target metrics address points under base limits");
            assert!(target_addr + METRICS_STRIDE <= ring_end, "FAIL: Target pointer overruns metrics partition layout");
            assert!(
                target_addr % std::mem::align_of::<NetworkMetricFrame>() as u64 == 0,
                "FAIL: Target metrics pointer is misaligned"
            );

            {
                let metrics_ptr = target_addr as *mut NetworkMetricFrame;
                let mut hardware_metrics: NetworkMetricFrame = unsafe { read_volatile(metrics_ptr) };

                if hardware_metrics.state_flags == 1 {
                    println!(
                        "[Metrics Watcher] Packets Processed: {}, Bytes Streamed: {}",
                        hardware_metrics.total_packets, hardware_metrics.total_bytes
                    );

                    self.packet_counter.store(hardware_metrics.total_packets, Ordering::Relaxed);

                    hardware_metrics.state_flags = 0;
                    unsafe {
                        write_volatile(metrics_ptr, hardware_metrics);
                    }
                }
            }

            self.ring_cursor = (self.ring_cursor + 1) % MAX_METRICS_SLOTS;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }
}

impl Drop for AsyncMetricsDaemon {
    fn drop(&mut self) {
        if self.ring_base != 0 {
            unsafe {
                libc::munmap(self.ring_base as *mut libc::c_void, METRICS_RING_SIZE as usize);
            }
        }
    }
}
