use std::path::PathBuf;

use tenx_lsp::analyzer::RustAnalyzer;

#[tokio::test]
async fn test_type_hint_basic() {
    let mut analyzer = RustAnalyzer::new();

    // Get the path to our example file
    let mut example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    example_path.push("tests/sample-project/src/main.rs");
    println!("{}", example_path.display());

    // Test getting type hint for the 'result' variable on line 37 (1-based)
    // line contents keeping indentation
    // 37 |    let result = calculate_average_age(&people);

    // This should be f64 from calculate_average_age function
    let result = analyzer
        .get_type_hint(example_path.to_str().unwrap(), 37, 12)
        .await;

    match result {
        Ok(Some(type_info)) => {
            println!("Got type info: {}", type_info);
            assert!(type_info.contains("f64"));
        }
        Ok(None) => {
            assert!(false);
        }
        Err(e) => {
            panic!("Error getting type hint: {}", e);
        }
    }
}

#[tokio::test]
async fn test_invalid_position() {
    let mut analyzer = RustAnalyzer::new();

    let mut example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    example_path.push("tests/sample-project/main.rs");

    // Test with invalid line/column (way beyond file bounds)
    let result = analyzer
        .get_type_hint(example_path.to_str().unwrap(), 9999, 9999)
        .await;

    // Should return an error for invalid position
    assert!(result.is_err());
}

#[tokio::test]
async fn test_nonexistent_file() {
    let mut analyzer = RustAnalyzer::new();

    // Test with non-existent file
    let result = analyzer.get_type_hint("/nonexistent/file.rs", 0, 0).await;

    // Should return an error for non-existent file
    assert!(result.is_err());
}
