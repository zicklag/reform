#[path = "../parser.rs"]
mod parser;

const LANG_REF: &str = include_str!("../../demo/lang.rf");

fn main() {
    match parser::parse_file(LANG_REF) {
        Ok(output) => {
            dbg!(output);
        }
        Err(e) => eprintln!("{e}"),
    }
}
