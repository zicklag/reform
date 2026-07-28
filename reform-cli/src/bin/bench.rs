use reform::engine::Engine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
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
    let path = PathBuf::from(&file);
    let main_src = std::fs::read_to_string(&path).unwrap();

    // Shared cache: lazily loads files on first access.
    let cache: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    let start = Instant::now();
    for _ in 0..n {
        let mut e = Engine::new();
        let cache = Arc::clone(&cache);
        e.register_command("load", Arc::new(move |engine, args| {
            let raw = args.first().map(|a| &**a).unwrap_or("");
            let path = match engine.base_dir() {
                Some(dir) => dir.join(raw),
                None => PathBuf::from(raw),
            };
            let normalized: PathBuf = path.components().collect();
            let key = normalized.to_string_lossy().to_string();
            let src = {
                let mut cache = cache.lock();
                match cache.get(&key) {
                    Some(s) => s.clone(),
                    None => {
                        let s = std::fs::read_to_string(&path)
                            .map_err(|e| anyhow::anyhow!("load {}: {e}", path.display()))?;
                        cache.insert(key.clone(), s.clone());
                        s
                    }
                }
            };
            let prev = engine.base_dir().map(|p| p.to_path_buf());
            engine.set_base_dir(normalized.parent().map(|p| p.to_path_buf()));
            let result = engine.load_str(&src);
            engine.set_base_dir(prev);
            result
        }));
        e.set_base_dir(path.parent().map(|p| p.to_path_buf()));
        e.load_str(&main_src).unwrap();
    }
    let elapsed = start.elapsed();
    let total_ms = elapsed.as_secs_f64() * 1000.0;
    let per_ms = total_ms / n as f64;
    println!(
        "{n} iterations in {total_ms:.1}ms — {per_ms:.3}ms per iteration"
    );
}
