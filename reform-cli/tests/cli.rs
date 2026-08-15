mod cli_tests {
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// Path to the compiled `reform` binary.
    fn reform() -> String {
        std::env::var("CARGO_BIN_EXE_reform").expect("CARGO_BIN_EXE_reform set")
    }

    /// Run the binary with the given stdin and return its stdout.
    fn run(stdin: &str, args: &[&str]) -> (String, String) {
        let mut child = Command::new(reform())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        {
            let mut stdin_h = child.stdin.take().expect("stdin");
            stdin_h.write_all(stdin.as_bytes()).expect("write");
        }
        let out = child.wait_with_output().expect("wait");
        (
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    }

    /// A startup file is loaded, then prompt input triggers a rule that prints.
    #[test]
    fn repl_prompts_become_prompt_facts() {
        let game = r#"
    $ rule on_look
        ( prompt look )
        ( $ println you see a cave )
    "#;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(game.as_bytes()).unwrap();
        let (out, _err) = run("look\n", &[file.path().to_str().unwrap()]);
        assert!(out.contains("youseeacave"), "stdout was: {out:?}");
    }

    /// By default, a `$`-prefixed line is inserted as a direct command.
    #[test]
    fn default_mode_runs_dollar_commands() {
        let (out, _err) = run("$ println hi\n", &[]);
        assert!(out.contains("hi"), "stdout was: {out:?}");
    }

    /// With `-s`, a `$`-prefixed line is just part of the prompt (not a command).
    #[test]
    fn safe_mode_treats_dollar_as_prompt_text() {
        let (out, _err) = run("$ println hi\n", &["-s"]);
        // No "hi" printed: it became a prompt fact `$ println hi`, not a command.
        assert!(!out.contains("hi"), "stdout was: {out:?}");
    }

    /// With `-s`, a non-`$` line is still a prompt.
    #[test]
    fn safe_mode_non_dollar_is_prompt() {
        let (out, _err) = run("look\n", &["-s"]);
        assert!(!out.contains("hi"), "stdout was: {out:?}");
    }

    /// A startup file that itself calls `quit` halts before the REPL.
    #[test]
    fn startup_quit_halts() {
        let game = r#"
    $ println started
    $ quit
    "#;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(game.as_bytes()).unwrap();
        let (out, _err) = run("look\n", &[file.path().to_str().unwrap()]);
        assert!(out.contains("started"), "stdout was: {out:?}");
        // The REPL never ran, so `look` produced no prompt processing output.
        assert!(!out.contains("you see a cave"), "stdout was: {out:?}");
    }

    /// The Cloak of Darkness: the spec's "test me" sequence wins.
    #[test]
    fn cloak_of_darkness_wins() {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        // CARGO_MANIFEST_DIR is reform-cli/; examples are at the workspace root.
        let path = format!("{dir}/../examples/cloak-of-darkness.rf");
        let (out, _err) = run(
            "s\nn\nw\ninventory\nhang cloak on hook\ne\ns\nread message\n",
            &[&path],
        );
        assert!(out.contains("You have won."), "stdout was: {out}");
        // The bar should be lit (described) on the second visit, and the cloak hung.
        assert!(
            out.contains("You hang the cloak on the hook."),
            "stdout was: {out}"
        );
    }

    /// Disturbing the message twice (then hanging the cloak) loses.
    #[test]
    fn cloak_of_darkness_loses_when_trampled() {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = format!("{dir}/../examples/cloak-of-darkness.rf");
        let (out, _err) = run(
            "s\nexamine bar\nexamine bar\nn\nw\nhang cloak on hook\ne\ns\nread message\n",
            &[&path],
        );
        assert!(out.contains("You have lost."), "stdout was: {out}");
    }
}
