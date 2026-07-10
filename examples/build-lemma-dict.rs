use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = Path::new("./data");

    // Collect all .conllu files
    let mut conllu_files: Vec<_> = fs::read_dir(data_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or_else(|| false, |ext| ext == "conllu")
        })
        .map(|e| e.path())
        .collect();
    conllu_files.sort();

    let mut entries: HashMap<String, String> = HashMap::new();

    for path in &conllu_files {
        let text = fs::read_to_string(path)?;
        let count = parse_conllu(&text, &mut entries);
        println!(
            "  {}: {} entries (total {})",
            path.file_name().unwrap().to_string_lossy(),
            count,
            entries.len()
        );
    }

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&entries)?;
    let mut compressed = Vec::with_capacity(bytes.len());
    let mut gze = flate2::write::GzEncoder::new(&mut compressed, Default::default());
    gze.write_all(&bytes)?;
    gze.finish()?;
    std::fs::write(data_dir.join("lemmas.rkyv.gz"), compressed)?;

    println!("\nTotal: {} entries", entries.len());
    println!("Output: {}/lemmas.rkyv.gz", data_dir.display());
    Ok(())
}

fn parse_conllu(text: &str, entries: &mut HashMap<String, String>) -> usize {
    let mut count = 0;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            continue;
        }

        let word = fields[1];
        let lemma = fields[2];
        let upos = fields[3];

        // Skip punctuation, symbols, and unknown
        if matches!(upos, "PUNCT" | "SYM" | "X") {
            continue;
        }

        // Skip URLs and emails
        if word.contains("http") || word.contains("mailto:") {
            continue;
        }

        let key = format!("{}|{}", word, upos);
        // Only insert if not already present (first seen wins)
        if let std::collections::hash_map::Entry::Vacant(e) = entries.entry(key) {
            e.insert(lemma.to_string());
            count += 1;
        }
    }

    count
}
