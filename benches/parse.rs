//! Reproducible dependency-free throughput measurements.
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn main() {
    // TODO: Add a larger document with mixed nesting and less repetitive strings.
    let records = format!(
        "[{}]",
        vec![r#"{"id":12345,"name":"Ada Lovelace","active":true,"tags":["rust","json"]}"#; 1024]
            .join(",")
    );
    let escaped = format!(
        "[{}]",
        vec![r#""hello\nworld\t\uD83E\uDD80""#; 4096].join(",")
    );
    let numbers = format!(
        "[{}]",
        (0..16384)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let cases = [
        ("records", records),
        ("escaped strings", escaped),
        ("integers", numbers),
    ];
    println!("strata-json: parse + drop; 200ms warmup; five 500ms samples; median throughput");
    for (name, input) in cases {
        let warmup = Instant::now();
        while warmup.elapsed() < Duration::from_millis(200) {
            black_box(strata_json::from_str(black_box(&input)).unwrap());
        }
        // TODO: Measure allocation counts separately from the timed runs.
        let mut rates = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            let mut iterations = 0u64;
            while start.elapsed() < Duration::from_millis(500) {
                black_box(strata_json::from_str(black_box(&input)).unwrap());
                iterations += 1;
            }
            rates.push(
                input.len() as f64 * iterations as f64
                    / start.elapsed().as_secs_f64()
                    / 1_048_576.0,
            );
        }
        rates.sort_by(f64::total_cmp);
        println!(
            "{name:16} {:7} bytes  {:8.2} MiB/s (range {:.2}–{:.2})",
            input.len(),
            rates[2],
            rates[0],
            rates[4]
        );
    }
}
