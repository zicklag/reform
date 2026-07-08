fn main() -> anyhow::Result<()> {
    let words = std::env::args()
        .skip(1)
        .map(|x| x.to_lowercase())
        .collect::<Vec<_>>();
    let x = words
        .iter()
        .map(|word| vec![crfs::Attribute::new(format!("form={word}"), 1.0)])
        .collect::<Vec<_>>();

    let model_data = std::fs::read("./data/upos.crfsuite")?;
    let model = crfs::Model::new(&model_data)?;
    let tagger = model.tagger()?;

    let result = tagger.tag(&x)?;

    for (word, pos) in words.iter().zip(result) {
        println!("{word} ( {pos} )");
    }

    Ok(())
}
