static INPUT: &str = include_str!("../../examples/lang.rf");

fn main() {
    match reform::parser::facts(INPUT) {
        Ok(facts) => {
            dbg!(facts);
        }
        Err(e) => eprintln!("{e}"),
    }
}
