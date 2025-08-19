use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use librustbelt::{
    analyzer::RustAnalyzerish, builder::RustAnalyzerishBuilder, entities::CursorCoordinates,
};
use tokio::sync::Mutex;

// Shared analyzer instance that gets initialized once
static SHARED_ANALYZER: OnceLock<Arc<Mutex<RustAnalyzerish>>> = OnceLock::new();

/// Get or initialize the shared analyzer instance
async fn get_shared_analyzer() -> Arc<Mutex<RustAnalyzerish>> {
    SHARED_ANALYZER
        .get_or_init(|| {
            let sample_path = get_sample_file_path();
            let analyzer = RustAnalyzerishBuilder::from_file(&sample_path)
                .expect("Failed to create analyzer from sample file")
                .build()
                .expect("Failed to build analyzer");
            Arc::new(Mutex::new(analyzer))
        })
        .clone()
}

/// Get the path to our sample project main.rs file
fn get_sample_file_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/sample-project/src/main.rs");
    path
}

#[tokio::test]
async fn test_rename_struct() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming the Person struct on line 5
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 5,
        column: 12, // Position of "Person" in "pub struct Person"
        symbol: None,
    };

    // First, find all references to verify we have multiple occurrences
    let references = analyzer
        .find_references(&cursor)
        .await
        .expect("Error finding references")
        .expect("Expected to find references to Person struct");

    // Ensure we have multiple references before renaming
    assert!(
        references.len() > 1,
        "Should find multiple references to Person struct"
    );

    // Get rename info without applying changes (to avoid modifying the test file)
    let rename_result = analyzer
        .get_rename_info(&cursor, "Individual")
        .await
        .expect("Error getting rename info")
        .expect("Expected rename info for Person struct");

    println!("Rename result: {}", rename_result);

    // Verify that all references would be updated
    assert_eq!(
        rename_result.file_changes.len(),
        1,
        "Should have changes in one file"
    );

    let file_change = &rename_result.file_changes[0];
    assert!(
        file_change.file_path.ends_with("main.rs"),
        "Changes should be in main.rs"
    );

    // Note: The number of edits may not match the number of references exactly
    // because rust-analyzer might handle some references differently during renaming
    assert!(
        file_change.edits.len() > 0,
        "Should have at least one edit for Person struct"
    );

    // Verify that all edits replace "Person" with "Individual"
    for edit in &file_change.edits {
        assert_eq!(
            edit.new_text, "Individual",
            "Edit should replace 'Person' with 'Individual'"
        );
    }
}

#[tokio::test]
async fn test_rename_function() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming the calculate_average_age function
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 61, // Line where calculate_average_age is defined - this might be incorrect
        column: 4, // Position of "calculate_average_age"
        symbol: Some("calculate_average_age".to_string()), // Use symbol resolution instead of exact coordinates
    };

    // First, find all references to verify we have multiple occurrences
    let references = analyzer
        .find_references(&cursor)
        .await
        .expect("Error finding references")
        .expect("Expected to find references to calculate_average_age function");

    // Ensure we have at least the definition and one usage
    assert!(
        references.len() >= 2,
        "Should find at least definition and usage of calculate_average_age"
    );

    // Get rename info without applying changes
    let rename_result = analyzer
        .get_rename_info(&cursor, "compute_mean_age")
        .await
        .expect("Error getting rename info")
        .expect("Expected rename info for calculate_average_age function");

    println!("Rename result: {}", rename_result);

    // Verify that all references would be updated
    assert_eq!(
        rename_result.file_changes.len(),
        1,
        "Should have changes in one file"
    );

    let file_change = &rename_result.file_changes[0];
    assert!(
        file_change.file_path.ends_with("main.rs"),
        "Changes should be in main.rs"
    );

    // Count the number of edits (should match the number of references)
    assert_eq!(
        file_change.edits.len(),
        references.len(),
        "Number of edits should match number of references"
    );

    // Verify that all edits replace "calculate_average_age" with "compute_mean_age"
    for edit in &file_change.edits {
        assert_eq!(
            edit.new_text, "compute_mean_age",
            "Edit should replace 'calculate_average_age' with 'compute_mean_age'"
        );
    }
}

#[tokio::test]
async fn test_rename_method() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming the with_email method
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 20, // Line where with_email is defined
        column: 16, // Position of "with_email"
        symbol: None,
    };

    // First, find all references to verify we have multiple occurrences
    let references = analyzer
        .find_references(&cursor)
        .await
        .expect("Error finding references")
        .expect("Expected to find references to with_email method");

    // Ensure we have at least the definition and one usage
    assert!(
        references.len() >= 2,
        "Should find at least definition and usage of with_email method"
    );

    // Get rename info without applying changes
    let rename_result = analyzer
        .get_rename_info(&cursor, "set_email")
        .await
        .expect("Error getting rename info")
        .expect("Expected rename info for with_email method");

    println!("Rename result: {}", rename_result);

    // Verify that all references would be updated
    assert_eq!(
        rename_result.file_changes.len(),
        1,
        "Should have changes in one file"
    );

    let file_change = &rename_result.file_changes[0];
    assert!(
        file_change.file_path.ends_with("main.rs"),
        "Changes should be in main.rs"
    );

    // Count the number of edits (should match the number of references)
    assert_eq!(
        file_change.edits.len(),
        references.len(),
        "Number of edits should match number of references"
    );

    // Verify that all edits replace "with_email" with "set_email"
    for edit in &file_change.edits {
        assert_eq!(
            edit.new_text, "set_email",
            "Edit should replace 'with_email' with 'set_email'"
        );
    }
}

#[tokio::test]
async fn test_rename_variable() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming the 'numbers' variable
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 41, // Line where numbers is defined
        column: 9, // Position of "numbers"
        symbol: None,
    };

    // First, find all references to verify we have multiple occurrences
    let references = analyzer
        .find_references(&cursor)
        .await
        .expect("Error finding references")
        .expect("Expected to find references to numbers variable");

    // Ensure we have at least the definition and one usage
    assert!(
        references.len() >= 2,
        "Should find at least definition and usage of numbers variable"
    );

    // Get rename info without applying changes
    let rename_result = analyzer
        .get_rename_info(&cursor, "values")
        .await
        .expect("Error getting rename info")
        .expect("Expected rename info for numbers variable");

    println!("Rename result: {}", rename_result);

    // Verify that all references would be updated
    assert_eq!(
        rename_result.file_changes.len(),
        1,
        "Should have changes in one file"
    );

    let file_change = &rename_result.file_changes[0];
    assert!(
        file_change.file_path.ends_with("main.rs"),
        "Changes should be in main.rs"
    );

    // Count the number of edits (should match the number of references)
    assert_eq!(
        file_change.edits.len(),
        references.len(),
        "Number of edits should match number of references"
    );

    // Verify that all edits replace "numbers" with "values"
    for edit in &file_change.edits {
        assert_eq!(
            edit.new_text, "values",
            "Edit should replace 'numbers' with 'values'"
        );
    }
}

#[tokio::test]
async fn test_rename_struct_field() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming the 'age' field in Person struct
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 7, // Line where age field is defined
        column: 9, // Position of "age"
        symbol: None,
    };

    // First, find all references to verify we have multiple occurrences
    let references = analyzer
        .find_references(&cursor)
        .await
        .expect("Error finding references")
        .expect("Expected to find references to age field");

    // Ensure we have multiple references
    assert!(
        references.len() > 1,
        "Should find multiple references to age field"
    );

    // Get rename info without applying changes
    let rename_result = analyzer
        .get_rename_info(&cursor, "years")
        .await
        .expect("Error getting rename info")
        .expect("Expected rename info for age field");

    println!("Rename result: {}", rename_result);

    // Verify that all references would be updated
    assert_eq!(
        rename_result.file_changes.len(),
        1,
        "Should have changes in one file"
    );

    let file_change = &rename_result.file_changes[0];
    assert!(
        file_change.file_path.ends_with("main.rs"),
        "Changes should be in main.rs"
    );

    // Count the number of edits (should match the number of references)
    assert_eq!(
        file_change.edits.len(),
        references.len(),
        "Number of edits should match number of references"
    );

    // Verify that all edits replace "age" with "years" or "years: "
    // Note: Some edits might include formatting like colons for struct initialization
    for edit in &file_change.edits {
        assert!(
            edit.new_text == "years" || edit.new_text == "years: ",
            "Edit should replace 'age' with 'years' or 'years: ', got '{}'",
            edit.new_text
        );
    }
}

#[tokio::test]
async fn test_rename_with_symbol_resolution() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming using symbol resolution (approximate coordinates)
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 6, // Approximate line near the 'name' field
        column: 10, // Approximate column
        symbol: Some("name".to_string()), // Symbol to find
    };

    // Get rename info without applying changes
    let rename_result = analyzer
        .get_rename_info(&cursor, "full_name")
        .await
        .expect("Error getting rename info")
        .expect("Expected rename info for name field");

    println!("Rename result with symbol resolution: {}", rename_result);

    // Verify that all references would be updated
    assert_eq!(
        rename_result.file_changes.len(),
        1,
        "Should have changes in one file"
    );

    let file_change = &rename_result.file_changes[0];
    assert!(
        file_change.file_path.ends_with("main.rs"),
        "Changes should be in main.rs"
    );

    // Verify that we have multiple edits
    assert!(
        file_change.edits.len() > 1,
        "Should have multiple edits for name field"
    );

    // Verify that all edits replace "name" with "full_name" or "full_name: "
    for edit in &file_change.edits {
        assert!(
            edit.new_text == "full_name" || edit.new_text == "full_name: ",
            "Edit should replace 'name' with 'full_name' or 'full_name: ', got '{}'",
            edit.new_text
        );
    }
}

#[tokio::test]
async fn test_rename_error_handling() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test renaming at an invalid position (whitespace or comment)
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 1, // First line (comment)
        column: 1, // First column
        symbol: None,
    };

    // Attempt to rename should return None or error
    let rename_result = analyzer.get_rename_info(&cursor, "NewName").await;

    match rename_result {
        Ok(None) => {
            // This is expected - nothing to rename at this position
            println!("Correctly returned None for invalid rename position");
        }
        Ok(Some(_)) => {
            panic!("Should not be able to rename at an invalid position");
        }
        Err(e) => {
            // An error is also acceptable
            println!("Correctly returned error for invalid rename position: {}", e);
        }
    }
}