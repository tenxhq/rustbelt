use librustbelt::entities::CursorCoordinates;

#[test]
fn test_cursor_resolution_finds_nothing() {
    let file_content = r#"
fn main() {
    println!("Hello, world!");
}
"#;

    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 2,
        column: 5,
        symbol: Some("nonexistent_symbol".to_string()),
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should return original coordinates when symbol not found
    assert_eq!(resolved.line, 2);
    assert_eq!(resolved.column, 5);
    assert_eq!(resolved.symbol, Some("nonexistent_symbol".to_string()));
}

#[test]
fn test_cursor_resolution_finds_closest_duplicate() {
    let file_content = r#"
fn test() {
    let value = 42;
    let another_value = value + 1;
    println!("value: {}", value);
}
"#;

    // Target the first occurrence of "value" on line 4
    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 4,
        column: 25,
        symbol: Some("value".to_string()),
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should find the first occurrence of "value" on line 4
    assert_eq!(resolved.line, 4);
    assert_eq!(resolved.column, 25); // First occurrence of "value" on line 4 (1-based)
}

#[test]
fn test_cursor_resolution_finds_closest_duplicate_second_occurrence() {
    let file_content = r#"
fn test() {
    let value = 42;
    let another_value = value + 1;
    println!("value: {}", value);
}
"#;

    // Target the second occurrence of "value" on line 5
    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 5,
        column: 26,
        symbol: Some("value".to_string()),
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should find the occurrence of "value" on line 5
    assert_eq!(resolved.line, 5);
    assert_eq!(resolved.column, 27); // "value" in the println! statement (1-based)
}

#[test]
fn test_cursor_resolution_multiple_symbols_same_line() {
    let file_content = r#"
fn test() {
    let result = calculate(value1, value2);
}
"#;

    // Target "value1" which is closer to column 24
    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 3,
        column: 24,
        symbol: Some("value1".to_string()),
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should find "value1"
    assert_eq!(resolved.line, 3);
    assert_eq!(resolved.column, 28); // Position of "value1"
}

#[test]
fn test_cursor_resolution_no_symbol_returns_original() {
    let file_content = r#"
fn main() {
    println!("Hello, world!");
}
"#;

    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 2,
        column: 5,
        symbol: None,
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should return original coordinates when no symbol specified
    assert_eq!(resolved.line, 2);
    assert_eq!(resolved.column, 5);
    assert_eq!(resolved.symbol, None);
}

#[test]
fn test_cursor_resolution_tolerance_box() {
    let file_content = r#"
fn main() {
    let x = 10;
    let y = 20;
    let z = 30;
    println!("x: {}", x);
}
"#;

    // Target line 6 but look for "x" which is on line 3
    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 6,
        column: 5,
        symbol: Some("x".to_string()),
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should find "x" on line 6 (within tolerance)
    assert_eq!(resolved.line, 6);
    assert_eq!(resolved.column, 15); // Position of "x" in the println! statement
}

#[test]
fn test_cursor_resolution_picks_closest_within_column_tolerance() {
    let file_content = r#"
fn test() {
    let foo = c(foo, foo_bar);
}
"#;

    // Target column 27 - this is closer to the second "foo" at column 30
    // than the first "foo" at column 21
    let cursor = CursorCoordinates {
        file_path: "/test/file.rs".to_string(),
        line: 3,
        column: 14,
        symbol: Some("foo".to_string()),
    };

    let resolved = cursor.resolve_coordinates(file_content);

    // Should find the second "foo" closer to target
    assert_eq!(resolved.line, 3);
    assert_eq!(resolved.column, 17);
}
