use std::path::Path;

use crfs::Attribute;

fn main() -> anyhow::Result<()> {
    let train_file = std::fs::File::open("./data/en_gum-ud-train.conllu")?;
    let train_data = conllu::parse_file(train_file)?;

    let mut trainer = crfs::Trainer::lbfgs();
    trainer.verbose(true);

    for sentence in train_data.iter() {
        let sentence = sentence.iter().as_slice();
        let mut x = Vec::with_capacity(sentence.len());
        let mut y = Vec::with_capacity(sentence.len());
        for token in sentence {
            let Some(upos) = token.upos else {
                continue;
            };
            let lemma = token.lemma.as_deref().unwrap_or("");
            let form = &token.form.to_lowercase();
            let capitalized = form
                .chars()
                .nth(1)
                .map(|x| x.is_uppercase())
                .unwrap_or(false);

            x.push(vec![
                Attribute::new(format!("lemma={lemma}"), 1.0),
                Attribute::new(format!("form={form}"), 1.0),
                Attribute::new("capitalized", if capitalized { 1.0 } else { 0.0 }),
            ]);
            y.push(upos.to_string());
        }
        trainer.append(&x, &y)?;
    }

    trainer.train(Path::new("./data/upos.crfsuite"))?;

    Ok(())
}
