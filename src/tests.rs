// src/tests.rs

// Allow dead code for helper functions or states only used in tests
#![allow(dead_code)] 

// Import items from your main crate (assuming main.rs, types.rs, spawners/* are in src/ or properly mod'd)
use crate::{App, ui}; // Assuming ui function is public or pub(crate) if tests are in same crate
use crate::types::AsyncResult; // Assuming these are public or pub(crate)
use crate::spawners::path_search::spawn_path_search; // Assuming this is public or pub(crate)

use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::timeout as tokio_timeout;

use std::time::Duration;
// For spawn_path_search tests
use std::fs;
use std::env;

// For App unit tests - helper
fn new_app() -> App {
    App::new()
}

// --- App Unit Tests ---
#[cfg(test)]
mod app_logic_tests {
    use super::*; // To get new_app(), App, AsyncResult etc.

    #[test]
    fn test_app_input_char() {
        let mut app = new_app();
        app.input.push('a');
        app.input.push('b');
        assert_eq!(app.input, "ab");
    }

    #[test]
    fn test_app_input_backspace() {
        let mut app = new_app();
        app.input.push('a');
        app.input.push('b');
        app.input.pop();
        assert_eq!(app.input, "a");
        app.input.pop();
        assert_eq!(app.input, "");
        app.input.pop(); // Pop on empty string
        assert_eq!(app.input, "");
    }

    #[test]
    fn test_app_history_add_and_limit() {
        let mut app = new_app();
        // Your KeyCode::Enter logic inserts at 0 and limits to 16
        for i in 0..20 {
            // Simulate how history is added in run_app
            let input_item = format!("item{}", i);
            app.history.insert(0, input_item.clone());
            if app.history.len() > 16 {
                app.history.pop();
            }
        }
        assert_eq!(app.history.len(), 16);
        assert_eq!(app.history[0], "item19"); // Newest
        assert_eq!(app.history[15], "item4"); // Oldest within limit
    }

    #[test]
    fn test_app_clear_prev() {
        let mut app = new_app();
        app.output = vec!["some output".to_string()];
        app.err_msg = "some error".to_string();
        app.spawner_results = vec!["some result".to_string()];
        
        app.clear_prev();
        
        assert!(app.output.is_empty());
        assert!(app.err_msg.is_empty());
        assert!(app.spawner_results.is_empty());
    }

    #[test]
    fn test_app_receive_spawner_results_success_multiple() {
        let mut app = new_app();
        let results = vec!["/path/to/dir1".to_string(), "/path/to/dir2".to_string()];
        
        app.is_loading = true;
        let async_result = AsyncResult::ZoxideResult(results.clone());

        // Simulate part of run_app's receiver logic
        match async_result {
            AsyncResult::ZoxideResult(received_results) => {
                app.spawner_results = received_results;
                app.err_msg.clear();
                if !app.spawner_results.is_empty() {
                    app.output = app.spawner_results.clone(); // You copy all results
                } else {
                    app.output = vec!["No results found".to_string()];
                }
            }
            _ => panic!("Unexpected async result type for success test"),
        }
        app.is_loading = false;

        assert!(!app.is_loading);
        assert_eq!(app.spawner_results, results);
        assert_eq!(app.output, results); // Should match all results
        assert!(app.err_msg.is_empty());
    }

    #[test]
    fn test_app_receive_spawner_results_empty() {
        let mut app = new_app();
        let results = Vec::<String>::new();
        
        app.is_loading = true;
        let async_result = AsyncResult::ZoxideResult(results.clone());

        match async_result {
            AsyncResult::ZoxideResult(received_results) => {
                app.spawner_results = received_results;
                app.err_msg.clear();
                if !app.spawner_results.is_empty() {
                    app.output = app.spawner_results.clone();
                } else {
                    app.output = vec!["No results found".to_string()];
                }
            }
            _ => panic!("Unexpected async result type for empty result test"),
        }
        app.is_loading = false;

        assert!(!app.is_loading);
        assert!(app.spawner_results.is_empty());
        assert_eq!(app.output, vec!["No results found".to_string()]);
        assert!(app.err_msg.is_empty());
    }

    #[test]
    fn test_app_receive_async_error() {
        let mut app = new_app();
        let error_message = "Test error from spawner".to_string();

        app.is_loading = true;
        let async_result = AsyncResult::Error(error_message.clone());

        match async_result {
            AsyncResult::Error(err_text) => {
                // This matches your run_app logic for AsyncResult::Error
                app.clear_prev(); 
                app.err_msg = err_text;
            }
            _ => panic!("Unexpected async result type for error test"),
        }
        app.is_loading = false; 

        assert!(!app.is_loading);
        assert_eq!(app.err_msg, error_message);
        assert!(app.output.is_empty());
        assert!(app.spawner_results.is_empty());
    }
}


// --- Spawner (path_search) Integration Tests ---
#[cfg(test)]
mod spawner_tests {
    use super::*; // To get spawn_path_search, AsyncResult etc.
    use tempfile::tempdir;
    // Note: tokio::test requires the tokio runtime. Ensure your main function or test runner handles this.
    // If you run `cargo test`, it should work with #[tokio::test] if tokio is a dependency.

    #[tokio::test]
    async fn test_spawn_path_search_direct_match() {
        let dir = tempdir().expect("Failed to create temp dir");
        let dir_path_str = dir.path().to_string_lossy().to_string();

        let (sender, mut receiver) = tokio_mpsc::channel::<AsyncResult>(1);
        
        spawn_path_search(dir_path_str.clone(), sender);

        match tokio_timeout(Duration::from_secs(3), receiver.recv()).await {
            Ok(Some(AsyncResult::ZoxideResult(results))) => { // tokio's recv() returns Option<T>
                assert!(results.contains(&dir_path_str), "Direct path '{}' not found in results: {:?}", dir_path_str, results);
            }
            Ok(Some(other)) => panic!("Expected ZoxideResult, got {:?}", other),
            Ok(None) => panic!("Channel closed unexpectedly (sender dropped without sending)"),
            Err(_) => panic!("Test for direct_match timed out"), // Timeout error from tokio_timeout
        }
    }

    #[tokio::test]
    async fn test_spawn_path_search_tilde_expansion_direct_match() {
        let home_dir_temp = tempdir().expect("Failed to create temp home dir");
        let target_dir_name = "test_cmds_for_tilde";
        let target_path_obj = home_dir_temp.path().join(target_dir_name);
        fs::create_dir(&target_path_obj).expect("Failed to create target dir in temp home");
        let expected_target_path_str = target_path_obj.to_string_lossy().to_string();
        
        let original_home = env::var("HOME").ok(); // Store original HOME, if set
        env::set_var("HOME", home_dir_temp.path().to_str().unwrap()); // Override HOME

        let query = format!("~/{}", target_dir_name);
        let (sender, mut receiver) = tokio_mpsc::channel(1);
        
        spawn_path_search(query.clone(), sender);

        let result_message = format!(
            "Tilde expanded path for query '{}' (expected approx: '{}') not found.", 
            query, expected_target_path_str
        );

        match tokio_timeout(Duration::from_secs(3), receiver.recv()).await {
            Ok(Some(AsyncResult::ZoxideResult(results))) => {
                assert!(results.contains(&expected_target_path_str), "{}. Results: {:?}", result_message, results);
            }
            Ok(Some(other)) => panic!("Expected ZoxideResult, got {:?}. {}", other, result_message),
            Ok(None) => panic!("Channel closed unexpectedly. {}", result_message),
            Err(_) => panic!("Test for tilde_expansion timed out. {}", result_message),
        }

        // Restore original HOME
        if let Some(home_val) = original_home {
            env::set_var("HOME", home_val);
        } else {
            env::remove_var("HOME");
        }
    }

    #[tokio::test]
    async fn test_spawn_path_search_non_existent_path() {
        // A path that's extremely unlikely to exist or be a zoxide match
        let non_existent_path = "/hopefully_non_existent_path_123abc/for_testing_xyz";
        let (sender, mut receiver) = tokio_mpsc::channel(1);
        
        spawn_path_search(non_existent_path.to_string(), sender);

        match tokio_timeout(Duration::from_secs(3), receiver.recv()).await {
            Ok(Some(AsyncResult::ZoxideResult(results))) => {
                assert!(results.is_empty(), "Expected no results for non-existent path, got {:?}", results);
            }
            Ok(Some(AsyncResult::Error(e))) => {
                 println!("[Test OK with Error] Received error for non-existent path (possibly zoxide not installed/configured, which is fine for this specific test focusing on not finding the direct path): {}", e);
                 // This is an acceptable outcome if zoxide errors but the non-existent path isn't found.
            }
            Ok(Some(other)) => panic!("Expected empty ZoxideResult or specific Error, got {:?}", other),
            Ok(None) => panic!("Channel closed unexpectedly for non_existent_path test"),
            Err(_) => panic!("Test for non_existent_path timed out"),
        }
    }
}

// --- UI (Buffer) Tests ---
#[cfg(test)]
mod ui_rendering_tests {
    use super::*; // To get App, ui
    use ratatui::Terminal; // Needed for Terminal::new
    use ratatui::backend::TestBackend;
    // Buffer is automatically brought in with TestBackend or prelude

    fn get_rendered_string(buffer: &ratatui::buffer::Buffer, x: u16, y: u16, width: u16) -> String {
        let mut s = String::new();
        for i in 0..width {
            // Use indexing and the symbol() method
            s.push_str(buffer[(x + i, y)].symbol());
        }
        s.trim_end().to_string() // Trim trailing spaces that might be empty cells
    }

    #[test]
    fn test_ui_renders_initial_empty_input() {
        let app = new_app();
        let backend = TestBackend::new(80, 10); // Width, Height
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| {
            ui(f, &app);
        }).unwrap();
        
        let buffer = terminal.backend().buffer();
        // Assuming input prompt like "> " at (1,1) within its block
        // Block borders take 1 char each side/top/bottom.
        // Input block is at main_layout[0], which is 3 lines high.
        // Search box title "Search"
        // Content "> " should be at (border_width + 1, border_height + 1) relative to block start.
        // Absolute coords: (x=1, y=1) for the content of the first cell of the paragraph.
        assert_eq!(buffer[(1, 1)].symbol(), ">", "Input prompt prefix '>' not found at (1,1)");
        assert_eq!(buffer[(2, 1)].symbol(), " ", "Input prompt space not found at (2,1)");
        // Check if other chars are empty for the rest of the input line if desired
    }

    #[test]
    fn test_ui_renders_loading_message_in_output() {
        let mut app = new_app();
        app.is_loading = true;
        
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        
        let buffer = terminal.backend().buffer();
        // Output block is main_layout[1], starts at row 3 (0-indexed) if previous block was 3 lines high.
        // Content of output block is at absolute y = 3 (block border) + 1 (content line) = 4.
        // We expect "Loading..." starting at x=1 (after border).
        let expected_text = "Loading...";
        let rendered_text = get_rendered_string(buffer, 1, 4, expected_text.len() as u16);
        assert_eq!(rendered_text, expected_text, "Loading message mismatch");
    }

    #[test]
    fn test_ui_renders_error_message_in_output() {
        let mut app = new_app();
        app.err_msg = "Test Error Occurred".to_string();
        
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        
        let buffer = terminal.backend().buffer();
        let expected_text = format!("Error: {}", app.err_msg);
        // Check at y=4, x=1
        let rendered_text = get_rendered_string(buffer, 1, 4, expected_text.len() as u16);
        assert_eq!(rendered_text, expected_text, "Error message mismatch");
    }

    #[test]
    fn test_ui_renders_results_in_output() {
        let mut app = new_app();
        app.output = vec!["/path/one".to_string(), "/path/two".to_string()];
        
        let backend = TestBackend::new(80, 10); // Enough height for 2 lines of output + blocks
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        
        let buffer = terminal.backend().buffer();
        // Expected path one at y=4, x=1
        let expected_text = &app.output[0];
        let rendered_text = get_rendered_string(buffer, 1, 4, expected_text.len() as u16);
        assert_eq!(rendered_text.as_str(), expected_text, "Single result line mismatch");

        // Test with multiple results, still only first should show due to block height
        app.output = vec!["/path/numero_uno".to_string(), "/path/numero_dos".to_string()];
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buffer_multi = terminal.backend().buffer();
        let expected_text_multi_first = &app.output[0];
        let rendered_text_multi_first = get_rendered_string(buffer_multi, 1, 4, expected_text_multi_first.len() as u16);
        assert_eq!(rendered_text_multi_first.as_str(), expected_text_multi_first, "First of multiple results mismatch");

    }
}