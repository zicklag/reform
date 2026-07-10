use std::{io::Write, path::Path};

use rkyv::{collections::swiss_table::ArchivedHashMap, rancor, string::ArchivedString};

type ArchivedDict = ArchivedHashMap<ArchivedString, ArchivedString>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = Path::new("./data");

    let compressed = std::fs::read(data_dir.join("lemmas.rkyv.gz"))?;
    let mut bytes = Vec::with_capacity(compressed.len());
    let mut gzd = flate2::write::GzDecoder::new(&mut bytes);
    gzd.write_all(&compressed)?;
    drop(compressed);
    gzd.finish()?;

    let lemmas = rkyv::access::<ArchivedDict, rancor::Error>(&bytes)?;

    for (k, v) in lemmas.iter() {
        println!("{k:25}{v}");
    }

    println!("\nTotal: {} entries", lemmas.len());
    Ok(())
}
