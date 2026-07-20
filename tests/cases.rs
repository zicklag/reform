use reform::engine::Engine;
use std::path::Path;

/// Load every `.rf` file in `tests/cases/` and verify it runs without error.
///
/// Each file is a self-contained reform program that uses `$ assert` / `$ assert-not`
/// / `$ panic` / `$ quit` to verify its own behavior. If any assertion fails or an
/// unexpected error occurs, the test fails with the file name and error message.
#[test]
fn reform_test_cases() {
    let cases_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let mut entries: Vec<_> = std::fs::read_dir(&cases_dir)
        .expect("tests/cases/ should exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rf"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    assert!(!entries.is_empty(), "no .rf files found in tests/cases/");

    for entry in entries {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let mut engine = Engine::new();
        let result = engine.load_file(&path);
        assert!(
            result.is_ok(),
            "reform test case in './tests/cases/{name}.rf' failed:\n\n{}",
            result.unwrap_err()
        );
    }
}
