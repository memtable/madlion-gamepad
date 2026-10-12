use std::time::{Duration, Instant};

use super::{BenchmarkArgs, open_keyboard, parse_key_list};
use crate::keyboard::plan_queries;

pub fn cmd_benchmark(args: BenchmarkArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut keyboard = open_keyboard(args.device.as_ref())?;
    let key_list = parse_key_list(&args.keys)?;
    if key_list.is_empty() {
        return Err("No keys specified for benchmark".into());
    }

    let queries = plan_queries(&key_list);

    println!("Benchmarking Madlion device: {}", keyboard.path().display());
    println!("Target keys: {:?}", key_list);
    println!(
        "Queries planned: {} (covers {} keys)",
        queries.len(),
        key_list.len()
    );
    println!("Running for {} second(s)... Please wait...", args.duration);

    let target_duration = Duration::from_secs(args.duration);
    let start_time = Instant::now();

    let mut latencies_us = Vec::new();
    let mut total_polls = 0u64;

    while start_time.elapsed() < target_duration {
        let t0 = Instant::now();
        let _ = keyboard.read_keys(&queries)?;
        let lat = t0.elapsed().as_micros() as u64;
        latencies_us.push(lat);
        total_polls += 1;
    }

    let total_time_s = start_time.elapsed().as_secs_f64();
    latencies_us.sort_unstable();

    let total_queries_sent = total_polls * (queries.len() as u64);
    let polls_per_sec = (total_polls as f64) / total_time_s;
    let queries_per_sec = (total_queries_sent as f64) / total_time_s;

    let min_lat = latencies_us.first().copied().unwrap_or(0);
    let max_lat = latencies_us.last().copied().unwrap_or(0);
    let avg_lat = if !latencies_us.is_empty() {
        (latencies_us.iter().sum::<u64>() as f64) / (latencies_us.len() as f64)
    } else {
        0.0
    };
    let p50 = percentile(&latencies_us, 50.0);
    let p95 = percentile(&latencies_us, 95.0);
    let p99 = percentile(&latencies_us, 99.0);

    println!("\n{:=<60}", "");
    println!("Benchmark Results");
    println!("{:=<60}", "");
    println!("Duration           : {:.3} s", total_time_s);
    println!("Total Polls        : {}", total_polls);
    println!("Total HID Packets  : {}", total_queries_sent);
    println!("Effective Poll Hz  : {:.2} Hz", polls_per_sec);
    println!("USB Packets / sec  : {:.2} pkt/s", queries_per_sec);
    println!("{:-<60}", "");
    println!("Roundtrip Latency (per full poll):");
    println!(
        "  Min              : {:6.2} us ({:.3} ms)",
        min_lat,
        min_lat as f64 / 1000.0
    );
    println!(
        "  Avg              : {:6.2} us ({:.3} ms)",
        avg_lat,
        avg_lat / 1000.0
    );
    println!(
        "  P50 (Median)     : {:6.2} us ({:.3} ms)",
        p50,
        p50 / 1000.0
    );
    println!(
        "  P95              : {:6.2} us ({:.3} ms)",
        p95,
        p95 / 1000.0
    );
    println!(
        "  P99              : {:6.2} us ({:.3} ms)",
        p99,
        p99 / 1000.0
    );
    println!(
        "  Max              : {:6.2} us ({:.3} ms)",
        max_lat,
        max_lat as f64 / 1000.0
    );
    println!("{:=<60}", "");

    Ok(())
}

fn percentile(sorted_data: &[u64], pct: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    let idx = ((pct / 100.0) * ((sorted_data.len() - 1) as f64)).round() as usize;
    sorted_data[idx] as f64
}
