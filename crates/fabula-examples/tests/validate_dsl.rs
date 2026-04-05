use std::fs;

#[test]
fn all_dsl_examples_parse() {
    let pattern = format!("{}/dsl/**/*.fabula", env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for entry in glob::glob(&pattern).unwrap() {
        let path = entry.unwrap();
        let content = fs::read_to_string(&path).unwrap();
        fabula_dsl::parse_document(&content).unwrap_or_else(|e| {
            panic!("{}: {e}", path.display());
        });
        count += 1;
    }
    assert!(count > 0, "no .fabula files found — check dsl/ directory");
}
