use reform::parser::parse_file;

const LANG_REF: &str = include_str!("../../demo/lang.rf");

fn main() {
    match parse_file(LANG_REF) {
        Ok(output) => {
            dbg!(output);
        }
        Err(e) => eprintln!("{e}"),
    }
}
