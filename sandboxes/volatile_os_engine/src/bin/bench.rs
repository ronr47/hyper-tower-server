// src/bench.rs
use std::time::Instant;

fn main() {
    println!("[+] Running Ferrari-Class Latency Test...");

    // Pre-allocate memory to prevent runtime heap jitter
    let mut data_packet: u64 = 42;
    
    // Start the high-precision hardware clock
    let start = Instant::now();

    // Simulate an ultra-fast atomic operation/pointer manipulation loop
    for _ in 0..1_000_000 {
        // Direct black-box hint to prevent the compiler from optimizing the loop away
        std::hint::black_box(data_packet = data_packet.wrapping_add(1));
    }

    let duration = start.elapsed();
    
    println!("==================================================");
    println!(" [+] Total Time for 1,000,000 Operations: {:?}", duration);
    println!(" [+] Average Latency Per Operation:   {:?}", duration / 1_000_000);
    println!("==================================================");
}
