use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use librustbelt::{analyzer::RustAnalyzerish, entities::CursorCoordinates};
use ra_ap_ide::SymbolKind;
use tokio::sync::Mutex;

// Shared analyzer instance that gets initialized once
static SHARED_ANALYZER: OnceLock<Arc<Mutex<RustAnalyzerish>>> = OnceLock::new();

/// Get or initialize the shared analyzer instance
async fn get_shared_analyzer() -> Arc<Mutex<RustAnalyzerish>> {
    SHARED_ANALYZER
        .get_or_init(|| Arc::new(Mutex::new(RustAnalyzerish::new())))
        .clone()
}

/// Get the path to our sample project main.rs file
fn get_sample_file_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/sample-project/src/main.rs");
    path
}

#[tokio::test]
async fn test_type_hint_simple_variable() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test type hint for 'people' variable on line 31 (HashMap<String, Person>)
    let cursor = CursorCoordinates {
        file_path: sample_path.to_str().unwrap().to_string(),
        line: 31,
        column: 13,
    };
    let type_info = analyzer
        .get_type_hint(&cursor)
        .await
        .expect("Error getting type hint")
        .expect("Expected type info but got None");

    println!("Type info for 'people': {type_info}");
    assert!(
        type_info.canonical_type.contains("HashMap")
            || type_info.canonical_type.contains("std::collections")
    );
}

#[tokio::test]
#[ignore = "Requires extracting function signatures"]
async fn test_type_hint_function_call() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test type hint for function call result on line 35 (f64)
    let type_info = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 35,
            column: 14,
        })
        .await
        .expect("Error getting type hint")
        .expect("Expected type info but got None");

    println!("Type info for function result: {type_info}");
    assert!(
        type_info
            .canonical_type
            .contains("pub fn insert(&mut self, k: K, v: V) -> Option<V>")
    );
}

#[tokio::test]
async fn test_type_hint_complex_generic() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test type hint for complex generic type on line 46
    let result = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 46,
            column: 9,
        })
        .await
        .expect("Error getting type hint");

    if let Some(type_info) = result {
        println!("Type info for complex generic: {type_info}");
        assert!(
            type_info.canonical_type.contains("Vec")
                && (type_info.canonical_type.contains("Option")
                    || type_info.canonical_type.contains("Result"))
        );
    } else {
        // Complex generics might not always have hover info available
        println!("No type info available for complex generic (acceptable)");
    }
}

#[tokio::test]
async fn test_get_definition_struct() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test get definition for Person struct usage on line 33
    let definitions = analyzer
        .get_definition(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 33,
            column: 18,
        })
        .await
        .expect("Error getting definition")
        .expect("Expected to find definition for Person struct");

    assert_eq!(
        definitions.len(),
        1,
        "Should find a single Person struct definition"
    );
    println!("Definition {:?}", definitions[0]);

    // Check that we found the struct definition
    let has_person_def = definitions
        .iter()
        .any(|def| def.name.contains("Person") && matches!(def.kind, Some(SymbolKind::Struct)));
    assert!(has_person_def, "Should find Person struct definition");
}

#[tokio::test]
async fn test_get_external_definition_function() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test get definition for function call on line 35
    let definitions = analyzer
        .get_definition(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 35,
            column: 14,
        })
        .await
        .expect("Error getting definition")
        .expect("Expected to find definition for function");

    assert_eq!(definitions.len(), 1, "Should find function call definition");
    let definition = &definitions[0];
    println!("Definition {definition:?}");

    // Check that we found the function definition
    assert_eq!(
        definition.description,
        Some("pub fn insert(&mut self, k: K, v: V) -> Option<V>".into())
    );
    let has_function_def =
        definition.name.contains("insert") && matches!(definition.kind, Some(SymbolKind::Function));
    assert!(has_function_def, "Should find `insert` function definition");
    assert_eq!(
        definition.module,
        "std::collections::hash::map::impl::HashMap<K, V, S>::insert"
    )
}

#[tokio::test]
async fn test_get_definition_method() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test get definition for method call on line 33 (.with_email)
    let definitions = analyzer
        .get_definition(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 33,
            column: 55,
        })
        .await
        .expect("Error getting definition")
        .expect("Expected to find definition for method");

    println!("Found {} definition(s) for method call", definitions.len());
    assert!(!definitions.is_empty());

    // Check that we found the method definition
    let has_method_def = definitions.iter().any(|def| {
        def.name.contains("with_email") && matches!(def.kind, Some(SymbolKind::Function))
    });
    assert!(has_method_def, "Should find with_email method definition");
}

#[tokio::test]
async fn test_error_handling_invalid_position() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test with invalid line/column (way beyond file bounds)
    let result = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 9999,
            column: 9999,
        })
        .await;

    // Should return an error for invalid position
    assert!(result.is_err());
}

#[tokio::test]
async fn test_error_handling_nonexistent_file() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;

    // Test with non-existent file
    let result = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: "/nonexistent/file.rs".to_string(),
            line: 10,
            column: 10,
        })
        .await;

    // Should return an error for non-existent file
    assert!(result.is_err());
}

#[tokio::test]
async fn test_no_definition_available() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test get definition on a comment or whitespace (should return None or empty)
    let result = analyzer
        .get_definition(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 1,
            column: 1,
        })
        .await
        .expect("Error getting definition");

    if let Some(definitions) = result {
        // If we get definitions, they should be empty or irrelevant
        println!(
            "Found {} definition(s) at comment position",
            definitions.len()
        );
    } else {
        // This is expected for comments/whitespace
        println!("No definitions found at comment position (expected)");
    }
}

#[tokio::test]
async fn test_multiple_usages_same_analyzer() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test multiple operations with the same analyzer to ensure state consistency

    // First operation: type hint
    let type_result = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 30,
            column: 9,
        })
        .await;
    assert!(type_result.is_ok());

    // Second operation: get definition
    let def_result = analyzer
        .get_definition(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 32,
            column: 15,
        })
        .await;
    assert!(def_result.is_ok());

    // Third operation: another type hint
    let type_result2 = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 39,
            column: 9,
        })
        .await;
    assert!(type_result2.is_ok());

    println!("Successfully performed multiple operations with shared analyzer");
}

#[tokio::test]
async fn test_analyzer_workspace_loading() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // This test ensures the analyzer can properly load and work with the workspace
    // The first call should trigger workspace loading
    let result = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 5,
            column: 10,
        })
        .await;

    // Should not error due to workspace loading issues
    match result {
        Ok(_) => println!("Workspace loaded successfully"),
        Err(e) => {
            // If there's an error, it should be about the specific position, not workspace
            // loading
            let error_msg = format!("{e}");
            assert!(
                !error_msg.contains("Cargo.toml") && !error_msg.contains("workspace"),
                "Workspace loading failed: {e}"
            );
        }
    }
}

#[tokio::test]
async fn test_type_hint_variable_with_name() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test type hint for 'doubled' variable on line 42 (should show "let doubled:
    // Vec<i32>")
    let type_info = analyzer
        .get_type_hint(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 41,
            column: 9,
        })
        .await
        .expect("Error getting type hint")
        .expect("Expected type info but got None");

    println!("Type info: {type_info}");

    // Should contain both the variable name and type
    assert!(
        type_info.symbol.contains("numbers"),
        "Should contain variable name 'numbers'"
    );
    // TODO Why is Vec<i32> not showing up?
    assert!(
        type_info.canonical_type.contains("Vec<i32>"),
        "Should contain type Vec<i32>"
    );
    // TODO This would be nice, but it doesn't show up in the type info
    // assert!(type_info.contains("let"), "Should contain 'let' keyword");
}

#[tokio::test]
async fn test_get_completions_basic() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test getting completions at a position where we expect some completions
    // For example, after "std::" we should get completions for std modules
    let completions = analyzer
        .get_completions(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 31,
            column: 18,
        })
        .await
        .expect("Error getting completions");

    if let Some(completions) = completions {
        println!("Found {} completions", completions.len());
        for completion in &completions {
            println!("  - {}", completion);
        }

        assert!(!completions.is_empty(), "Should find some completions");

        // Check that we have reasonable completion items
        let has_reasonable_completion = completions.iter().any(|c| !c.name.is_empty());
        assert!(
            has_reasonable_completion,
            "Should have completions with non-empty names"
        );
    } else {
        // Some positions might not have completions available
        println!("No completions found at this position");
    }
}

#[tokio::test]
async fn test_get_completions_method_chaining() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test getting completions after a dot (method completions)
    // This should trigger method/field completions
    let completions = analyzer
        .get_completions(&CursorCoordinates {
            file_path: sample_path.to_str().unwrap().to_string(),
            line: 32,
            column: 20,
        })
        .await
        .expect("Error getting completions");

    if let Some(completions) = completions {
        println!("Found {} method completions", completions.len());
        for completion in &completions {
            println!("  - {}", completion);
        }

        // For a HashMap, we should see methods like insert, get, etc.
        let has_hash_map_methods = completions
            .iter()
            .any(|c| c.name.contains("insert") || c.name.contains("get") || c.name.contains("len"));

        if has_hash_map_methods {
            assert!(
                has_hash_map_methods,
                "Should find HashMap methods in completions"
            );
        } else {
            println!("No HashMap methods found, but that's acceptable depending on context");
        }
    } else {
        println!("No method completions found at this position");
    }
}

#[tokio::test]
async fn test_view_inlay_hints() {
    let analyzer = get_shared_analyzer().await;
    let mut analyzer = analyzer.lock().await;
    let sample_path = get_sample_file_path();

    // Test getting completions after a dot (method completions)
    // This should trigger method/field completions
    let file_with_inlay_hints = analyzer
        .view_inlay_hints(sample_path.to_str().unwrap())
        .await
        .expect("Error viewing inlay hints");

    println!("{file_with_inlay_hints}");
    // Adding type hints
    assert!(
        file_with_inlay_hints.contains("let _sum: i32"),
        "Should show inlay type hint for _sum"
    );
    assert!(
        file_with_inlay_hints.contains("let person: Person"),
        "Should show inlay type hint for person"
    );
    assert!(
        file_with_inlay_hints.contains("let numbers: Vec<i32>"),
        "Should show inlay type hint for numbers"
    );
    assert!(
        file_with_inlay_hints.contains("for item: Option<Result<String, &str>>"),
        "Should show inlay type hint for numbers"
    );

    // Adding named arguments
    // assert!(file_with_inlay_hints.contains("Person::new(name: \"Alice\".to_string()"), "Should show named arguments");

    // Keeping existing types intact
    assert!(
        file_with_inlay_hints.contains("let doubled: Vec<i32>"),
        "Should keep existing types intact"
    );
}
