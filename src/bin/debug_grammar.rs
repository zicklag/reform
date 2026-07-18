use reform::rule::Rule;

static INPUT: &str = include_str!("../../examples/lang.rf");

fn main() {
    let facts = match reform::parser::facts(INPUT) {
        Ok(facts) => facts,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    println!("=== Facts ===");
    for fact in &facts {
        dbg!(&fact);
    }

    println!("\n=== Rules ===");
    for fact in &facts {
        // A rule fact is either ["rule", name, pattern, body] or
        // ["$", "rule", name, pattern, body] (with the $ prefix not yet stripped).
        let rule_args: Option<&[reform::Arg]> = {
            let s = fact.as_slice();
            if s.len() == 5 && &*s[0] == "$" && &*s[1] == "rule" {
                Some(&s[1..])
            } else if s.len() == 4 && &*s[0] == "rule" {
                Some(s)
            } else {
                None
            }
        };

        if let Some(args) = rule_args {
            match Rule::parse(args) {
                Ok(rule) => println!("  {rule}"),
                Err(e) => eprintln!("  failed to parse rule {fact:?}: {e:?}"),
            }
        }
    }
}
