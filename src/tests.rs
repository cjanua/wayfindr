// src/tests.rs

// Allow dead code for helper functions or states only used in tests
#![allow(dead_code)]

// Import items from your main crate (assuming main.rs, types.rs, spawners/* are in src/ or properly mod'd)
use crate::{App, ui}; // Assuming ui function is public or pub(crate) if tests are in same crate
use crate::types::{ActionResult, AsyncResult}; // Assuming these are public or pub(crate)
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
    use crate::types::AppError;

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
        app.output = vec![ActionResult {
            spawner: "test".to_string(),
            action: "do".to_string(),
            description: "some output".to_string(),
            data: "".to_string(),
        }];
        app.err_msg = "some error".to_string();
        // app.spawner_results is no longer directly used for output
        // app.spawner_results = vec!["some result".to_string()];

        app.clear_prev();

        assert!(app.output.is_empty());
        assert!(app.err_msg.is_empty());
        // assert!(app.spawner_results.is_empty());
    }

    #[test]
    fn test_app_receive_path_search_results_success_multiple() {
        let mut app = new_app();
        let results = vec![
            ActionResult {
                spawner: "z".to_string(),
                action: "cd".to_string(),
                description: "/path/to/dir1".to_string(),
                data: "/path/to/dir1".to_string(),
            },
            ActionResult {
                spawner: "fs".to_string(),
                action: "cd".to_string(),
                description: "/path/to/dir2".to_string(),
                data: "/path/to/dir2".to_string(),
            },
        ];

        app.is_loading = true;
        let async_result = AsyncResult::PathSearchResult(results.clone());

        // Simulate part of run_app's receiver logic
        match async_result {
            AsyncResult::PathSearchResult(received_results) => {
                app.output = received_results;
                app.err_msg.clear();
            }
            _ => panic!("Unexpected async result type for success test"),
        }
        app.is_loading = false;

        assert!(!app.is_loading);
        assert_eq!(app.output.len(), results.len());
        assert_eq!(app.output[0].description, results[0].description);
        assert_eq!(app.output[1].description, results[1].description);
        assert!(app.err_msg.is_empty());
    }

    #[test]
    fn test_app_receive_path_search_results_empty() {
        let mut app = new_app();
        let results = Vec::<ActionResult>::new();

        app.is_loading = true;
        let async_result = AsyncResult::PathSearchResult(results.clone());

        match async_result {
            AsyncResult::PathSearchResult(received_results) => {
                app.output = received_results;
                app.err_msg.clear();
            }
            _ => panic!("Unexpected async result type for empty result test"),
        }
        app.is_loading = false;

        assert!(!app.is_loading);
        assert!(app.output.is_empty());
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
    }

    #[tokio::test]
    async fn test_app_execute_action_cd_zoxide() {
        let app = new_app();
        let action_result = ActionResult {
            spawner: "z".to_string(),
            action: "cd".to_string(),
            description: "/test/path".to_string(),
            data: "/test/path".to_string(),
        };
        let result = app.execute_action(&action_result).await;
        assert!(result.is_ok());
        // You might want to capture stdout to check the printed message if needed
    }

    #[tokio::test]
    async fn test_app_execute_action_unknown_spawner() {
        let app = new_app();
        let action_result = ActionResult {
            spawner: "unknown".to_string(),
            action: "do".to_string(),
            description: "something".to_string(),
            data: "".to_string(),
        };
        let result = app.execute_action(&action_result).await;
        assert!(result.is_err());
        if let Err(AppError::ActionError(msg)) = result {
            assert!(msg.contains("Unknown spawner"));
        } else {
            panic!("Expected ActionError for unknown spawner");
        }
    }

    #[tokio::test]
    async fn test_app_execute_action_unknown_action() {
        let app = new_app();
        let action_result = ActionResult {
            spawner: "z".to_string(),
            action: "open".to_string(),
            description: "/test/file".to_string(),
            data: "/test/file".to_string(),
        };
        let result = app.execute_action(&action_result).await;
        assert!(result.is_err());
        if let Err(AppError::ActionError(msg)) = result {
            assert!(msg.contains("Unknown action"));
        } else {
            panic!("Expected ActionError for unknown action");
        }
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
            Ok(Some(AsyncResult::PathSearchResult(results))) => {
                assert!(
                    results.iter().any(|r| r.data == dir_path_str),
                    "Direct path '{}' not found in results: {:?}",
                    dir_path_str,
                    results
                );
                assert!(
                    results.iter().any(|r| r.spawner == "fs" && r.action == "cd"),
                    "Expected 'fs cd' action for direct path, got {:?}",
                    results
                );
            }
            Ok(Some(other)) => panic!("Expected PathSearchResult, got {:?}", other),
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
            Ok(Some(AsyncResult::PathSearchResult(results))) => {
                assert!(
                    results.iter().any(|r| r.data == expected_target_path_str),
                    "{}. Results: {:?}",
                    result_message,
                    results
                );
                assert!(
                    results.iter().any(|r| r.spawner == "fs" && r.action == "cd"),
                    "Expected 'fs cd' action for tilde expanded path, got {:?}",
                    results
                );
            }
            Ok(Some(other)) => panic!("Expected PathSearchResult, got {:?}. {}", other, result_message),
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
            Ok(Some(AsyncResult::PathSearchResult(results))) => {
                assert!(results.is_empty(), "Expected no results for non-existent path, got {:?}", results);
            }
            Ok(Some(AsyncResult::Error(e))) => {
                println!("[Test OK with Error] Received error for non-existent path (possibly zoxide not installed/configured, which is fine for this specific test focusing on not finding the direct path): {}", e);
                // This is an acceptable outcome if zoxide errors but the non-existent path isn't found.
            }
            Ok(Some(other)) => panic!("Expected empty PathSearchResult or specific Error, got {:?}", other),
            Ok(None) => panic!("Channel closed unexpectedly for non_existent_path test"),
            Err(_) => panic!("Test for non_existent_path timed out"),
        }
    }
}

// --- UI (Buffer) Tests ---
#[cfg(test)]
mod ui_rendering_tests {
    use std::rc::Rc;

    use super::*; use ratatui::layout::{Constraint, Direction, Layout, Rect};
    // To get App, ui
    use ratatui::Terminal; // Needed for Terminal::new
    use ratatui::backend::TestBackend;
    // Buffer is automatically brought in with TestBackend or prelude

    fn get_rendered_string_at_line(buffer: &ratatui::buffer::Buffer, area: Rect, line_index: u16) -> String {
        let mut s = String::new();
        let content_y = area.y + 1 + line_index; // +1 for top border
        if content_y >= area.bottom() - 1 { // Check if line_index is out of content bounds
            return s; // Return empty string if trying to read outside content area
        }
        for x in area.x + 1..area.right() - 1 { // +1 for left border, -1 for right border
            s.push_str(buffer[(x, content_y)].symbol());
        }
        s.trim_end().to_string()
    }

    // Helper function to get all content lines from a block
    fn get_rendered_block_content(buffer: &ratatui::buffer::Buffer, block_area: Rect) -> String {
        let mut lines = Vec::new();
        let content_height = block_area.height.saturating_sub(2); // Height for content (excluding borders)
        for i in 0..content_height {
            lines.push(get_rendered_string_at_line(buffer, block_area, i));
        }
        lines.join("\n")
    }


    #[test]
    fn test_ui_renders_initial_empty_input() {
        let app = new_app();
        let backend = TestBackend::new(80, 10); // Width, Height
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|f| {
            ui(f, &app);
        })
        .unwrap();

        let buffer = terminal.backend().buffer();
        let input_block_area = test_main_layout(Rect::new(0,0,80,10))[0];
        // Input prompt "> " at (1,1) relative to block's content area.
        assert_eq!(buffer[(input_block_area.x + 1, input_block_area.y + 1)].symbol(), ">", "Input prompt prefix '>' not found");
        assert_eq!(buffer[(input_block_area.x + 2, input_block_area.y + 1)].symbol(), " ", "Input prompt space not found");
    }

    #[test]
    fn test_ui_renders_loading_message_in_output() {
        let mut app = new_app();
        app.is_loading = true;

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let output_block_area = test_main_layout(Rect::new(0,0,80,10))[1];
        let rendered_text = get_rendered_string_at_line(buffer, output_block_area, 0);
        assert_eq!(rendered_text, "Loading...", "Loading message mismatch");
    }

    #[test]
    fn test_ui_renders_error_message_in_output() {
        let mut app = new_app();
        app.err_msg = "Test Error Occurred".to_string();

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let output_block_area = test_main_layout(Rect::new(0,0,80,10))[1];
        let rendered_text = get_rendered_string_at_line(buffer, output_block_area, 0);
        let expected_text = format!("Error: {}", app.err_msg);
        assert_eq!(rendered_text, expected_text, "Error message mismatch");
    }

    #[test]
    fn test_ui_renders_results_in_output() {
        let mut app = new_app();
        app.output = vec![
            ActionResult {
                spawner: "z".to_string(),
                action: "cd".to_string(),
                description: "/path/numero_uno".to_string(),
                data: "/path/numero_uno".to_string(),
            },
            ActionResult {
                spawner: "fs".to_string(),
                action: "cd".to_string(),
                description: "/path/numero_dos".to_string(),
                data: "/path/numero_dos".to_string(),
            },
        ];

        // Ensure backend and layout provide enough space for 2 lines of content in output
        let backend = TestBackend::new(80, 15); // Total height 15
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();

        // Use the test_main_layout which now gives more space to output
        let output_block_area = test_main_layout(Rect::new(0, 0, 80, 15))[1];

        // This helper function reads all content lines from the block
        let rendered_content = get_rendered_block_content(buffer, output_block_area);

        let expected_line_one = "[z] cd /path/numero_uno";
        let expected_line_two = "[fs] cd /path/numero_dos";

        // Check if the full rendered content (with newlines) contains each expected line
        assert!(
            rendered_content.contains(expected_line_one),
            "Output should contain: \"{}\". Full output: \n\"{}\"", expected_line_one, rendered_content
        );
        assert!(
            rendered_content.contains(expected_line_two),
            "Output should contain: \"{}\". Full output: \n\"{}\"", expected_line_two, rendered_content
        );

        // For a more precise check if you expect them on separate lines:
        let lines: Vec<&str> = rendered_content.split('\n').collect();
        assert!(lines.len() >= 2, "Expected at least 2 lines of output, got {}. Content: \n\"{}\"", lines.len(), rendered_content);
        assert_eq!(lines[0], expected_line_one, "First line of output mismatch. Content: \n\"{}\"", rendered_content);
        assert_eq!(lines[1], expected_line_two, "Second line of output mismatch. Content: \n\"{}\"", rendered_content);

    }

    // Renamed to avoid confusion with any layout in main.rs and updated Constraint
    fn test_main_layout(area: Rect) -> Rc<[Rect]> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Input area
                Constraint::Length(4), // Output area (changed from 3 to 4 to allow 2 content lines)
                Constraint::Min(0),    // History area
            ])
            .split(area)
    }
}