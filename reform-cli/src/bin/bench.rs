use reform::engine::Engine;
use std::path::Path;

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
    for _ in 0..n {
        let mut e = Engine::new();
        e.load_file(path).unwrap();
    }
}