use reform::engine::Engine;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let file = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "examples/demo-3.rf".to_string());
    let path = Path::new(&file);
    let start = Instant::now();
    for _ in 0..n {
        let mut e = Engine::new();
        e.load_file(path).unwrap();
    }
    let elapsed = start.elapsed();
    let total_ms = elapsed.as_secs_f64() * 1000.0;
    let per_ms = total_ms / n as f64;
    println!(
        "{n} iterations in {total_ms:.1}ms — {per_ms:.3}ms per iteration"
    );
}