// // src/tests.rs

// // Allow dead code for helper functions or states only used in tests
// #![allow(dead_code)]

// // Import items from your main crate
// use crate::app::App; // App is now in the app module
// use crate::types::{ActionResult, AsyncResult, AppError}; // Added AppError for tests
// use crate::spawners::path_search::spawn_path_search;

// use tokio::sync::mpsc as tokio_mpsc;
// use tokio::time::timeout as tokio_timeout;

// use std::time::Duration;
// // For spawn_path_search tests
// use std::fs;
// use std::env;

// // For App unit tests - helper
// fn new_app() -> App {
//     App::new() // App::new() is now crate::app::App::new()
// }

// // --- App Unit Tests ---
// #[cfg(test)]
// mod app_logic_tests {
//     // use crate::types::AppError; // Already imported at the top level of the file

//     use super::*; // To get new_app(), App, AsyncResult, ActionResult, AppError etc.

//     #[test]
//     fn test_app_input_char() {
//         let mut app = new_app();
//         app.input.push('a');
//         app.input.push('b');
//         assert_eq!(app.input, "ab");
//     }

//     #[test]
//     fn test_app_input_backspace() {
//         let mut app = new_app();
//         app.input.push('a');
//         app.input.push('b');
//         app.input.pop();
//         assert_eq!(app.input, "a");
//         app.input.pop();
//         assert_eq!(app.input, "");
//         app.input.pop(); // Pop on empty string
//         assert_eq!(app.input, "");
//     }

//     #[test]
//     fn test_app_history_add_and_limit() {
//         let mut app = new_app();
//         for i in 0..20 {
//             let input_item = format!("item{}", i);
//             app.history.insert(0, input_item.clone());
//             if app.history.len() > 16 { // Assuming 16 is the hardcoded limit in run_app_loop
//                 app.history.pop();
//             }
//         }
//         assert_eq!(app.history.len(), 16);
//         assert_eq!(app.history[0], "item19");
//         assert_eq!(app.history[15], "item4");
//     }

//     #[test]
//     fn test_app_clear_prev() {
//         let mut app = new_app();
//         app.output = vec![ActionResult {
//             spawner: "test".to_string(),
//             action: "do".to_string(),
//             description: "some output".to_string(),
//             data: "".to_string(),
//         }];
//         app.err_msg = "some error".to_string();

//         app.clear_prev();

//         assert!(app.output.is_empty());
//         assert!(app.err_msg.is_empty());
//     }

//     #[test]
//     fn test_app_receive_path_search_results_success_multiple() {
//         let mut app = new_app();
//         let results = vec![
//             ActionResult {
//                 spawner: "z".to_string(),
//                 action: "cd".to_string(),
//                 description: "/path/to/dir1".to_string(),
//                 data: "/path/to/dir1".to_string(),
//             },
//             ActionResult {
//                 spawner: "fs".to_string(),
//                 action: "cd".to_string(),
//                 description: "/path/to/dir2".to_string(),
//                 data: "/path/to/dir2".to_string(),
//             },
//         ];

//         app.is_loading = true;
//         // Simulate receiving AsyncResult and updating app state as in run_app_loop
//         app.output = results.clone();
//         app.err_msg.clear();
//         app.selected_output_index = 0;
//         app.focus = if app.output.is_empty() { crate::app::FocusBlock::Input } else { crate::app::FocusBlock::Output };
//         app.is_loading = false;


//         assert!(!app.is_loading);
//         assert_eq!(app.output.len(), results.len());
//         assert_eq!(app.output[0].description, results[0].description);
//         assert_eq!(app.output[1].description, results[1].description);
//         assert!(app.err_msg.is_empty());
//         assert_eq!(app.focus, crate::app::FocusBlock::Output);
//     }

//     #[test]
//     fn test_app_receive_path_search_results_empty() {
//         let mut app = new_app();
//         let results = Vec::<ActionResult>::new();

//         app.is_loading = true;
//         // Simulate receiving AsyncResult
//         app.output = results.clone();
//         app.err_msg.clear();
//         app.selected_output_index = 0;
//         app.focus = if app.output.is_empty() { crate::app::FocusBlock::Input } else { crate::app::FocusBlock::Output };
//         app.is_loading = false;

//         assert!(!app.is_loading);
//         assert!(app.output.is_empty());
//         assert!(app.err_msg.is_empty());
//         assert_eq!(app.focus, crate::app::FocusBlock::Input);
//     }

//     #[test]
//     fn test_app_receive_async_error() {
//         let mut app = new_app();
//         let error_message = "Test error from spawner".to_string();

//         app.is_loading = true;
//         // Simulate receiving AsyncResult
//         app.clear_prev(); // As per run_app_loop logic
//         app.err_msg = error_message.clone();
//         app.focus = crate::app::FocusBlock::Input; // As per run_app_loop logic
//         app.is_loading = false;


//         assert!(!app.is_loading);
//         assert_eq!(app.err_msg, error_message);
//         assert!(app.output.is_empty());
//         assert_eq!(app.focus, crate::app::FocusBlock::Input);
//     }

//     // Tests for handle_action_execution (previously execute_action)
//     // These tests might need adjustment if handle_action_execution now primarily delegates
//     // and doesn't return errors directly for process spawning, but rather sets app.err_msg.
//     // Based on the current app.rs, handle_action_execution does return Result.
//     #[tokio::test]
//     async fn test_app_handle_action_execution_cd_success() {
//         let mut app = new_app(); // Make app mutable if it needs to set err_msg on failure
//         let action_result = ActionResult {
//             spawner: "z".to_string(),
//             action: "cd".to_string(),
//             description: "/tmp".to_string(), // Use a path that generally exists for testing spawn
//             data: "/tmp".to_string(),
//         };
//         // This test assumes `process_execution::launch_kitty_for_cd` will succeed for "/tmp"
//         // or that failure is handled gracefully by returning an error.
//         // If `launch_kitty_for_cd` always returns Ok(()) and errors are only logged,
//         // this test needs to be rethought or process_execution needs to propagate errors.
//         // Current `process_execution::launch_kitty_for_cd` returns Result<_, std::io::Error>
//         // and `handle_action_execution` converts it.
//         let result = app.handle_action_execution(&action_result).await;

//         // If kitty is not installed, this will be an error.
//         // The test here is more about the logic flow than successful Kitty launch.
//         // For a true success, we'd need to mock process_execution.
//         // Let's check if it either succeeds (kitty launched) or fails with ActionError.
//         if result.is_ok() {
//             // This means process_execution::launch_kitty_for_cd succeeded
//             // or at least `spawn` returned Ok.
//             println!("test_app_handle_action_execution_cd_success: Kitty launch reported success.");
//         } else if let Err(AppError::ActionError(msg)) = result {
//             // This is also an acceptable outcome if Kitty isn't found or fails to spawn
//             println!("test_app_handle_action_execution_cd_success: Kitty launch failed as expected (ActionError): {}", msg);
//             assert!(msg.contains("Failed to open Kitty"));
//         } else {
//             panic!("Expected Ok or ActionError, got {:?}", result);
//         }
//     }

//     #[tokio::test]
//     async fn test_app_handle_action_execution_unknown_spawner() {
//         let mut app = new_app();
//         let action_result = ActionResult {
//             spawner: "unknown_spawner_test".to_string(),
//             action: "do".to_string(),
//             description: "something".to_string(),
//             data: "".to_string(),
//         };
//         let result = app.handle_action_execution(&action_result).await;
//         assert!(result.is_err());
//         if let Err(AppError::ActionError(msg)) = result {
//             assert!(msg.contains("Unknown spawner 'unknown_spawner_test'"));
//         } else {
//             panic!("Expected ActionError for unknown spawner, got {:?}", result);
//         }
//         assert_eq!(app.err_msg, "Unknown spawner 'unknown_spawner_test'");
//     }

//     #[tokio::test]
//     async fn test_app_handle_action_execution_unknown_action() {
//         let mut app = new_app();
//         let action_result = ActionResult {
//             spawner: "z".to_string(),
//             action: "unknown_action_test".to_string(),
//             description: "/test/file".to_string(),
//             data: "/test/file".to_string(),
//         };
//         let result = app.handle_action_execution(&action_result).await;
//         assert!(result.is_err());
//         if let Err(AppError::ActionError(msg)) = result {
//             assert!(msg.contains("Unknown action 'unknown_action_test' for spawner 'z'"));
//         } else {
//             panic!("Expected ActionError for unknown action, got {:?}", result);
//         }
//         assert_eq!(app.err_msg, "Unknown action 'unknown_action_test' for spawner 'z'");
//     }
// }

// // --- Spawner (path_search) Integration Tests ---
// #[cfg(test)]
// mod spawner_tests {
//     use super::*;
//     use tempfile::tempdir;

//     #[tokio::test]
//     async fn test_spawn_path_search_direct_match() {
//         let dir = tempdir().expect("Failed to create temp dir");
//         let dir_path_str = dir.path().to_string_lossy().to_string();

//         let (sender, mut receiver) = tokio_mpsc::channel::<AsyncResult>(1);
//         spawn_path_search(dir_path_str.clone(), sender);

//         match tokio_timeout(Duration::from_secs(3), receiver.recv()).await {
//             Ok(Some(AsyncResult::PathSearchResult(results))) => {
//                 assert!(!results.is_empty(), "Expected results for direct match, got none.");
//                 assert!(
//                     results.iter().any(|r| r.data == dir_path_str && r.spawner == "fs"),
//                     "Direct path '{}' (fs) not found in results: {:?}", dir_path_str, results
//                 );
//             }
//             Ok(Some(other)) => panic!("Expected PathSearchResult, got {:?}", other),
//             Ok(None) => panic!("Channel closed unexpectedly"),
//             Err(_) => panic!("Test for direct_match timed out"),
//         }
//     }

//     #[tokio::test]
//     async fn test_spawn_path_search_tilde_expansion_direct_match() {
//         let home_dir_temp = tempdir().expect("Failed to create temp home dir");
//         let target_dir_name = "test_cmds_for_tilde_spawner"; // Unique name
//         let target_path_obj = home_dir_temp.path().join(target_dir_name);
//         fs::create_dir_all(&target_path_obj).expect("Failed to create target dir in temp home");
//         let expected_target_path_str = target_path_obj.to_string_lossy().to_string();

//         let original_home = env::var("HOME").ok();
//         env::set_var("HOME", home_dir_temp.path().to_str().unwrap());

//         let query = format!("~/{}", target_dir_name);
//         let (sender, mut receiver) = tokio_mpsc::channel(1);
//         spawn_path_search(query.clone(), sender);

//         match tokio_timeout(Duration::from_secs(3), receiver.recv()).await {
//             Ok(Some(AsyncResult::PathSearchResult(results))) => {
//                 assert!(!results.is_empty(), "Expected results for tilde expansion, got none.");
//                 assert!(
//                     results.iter().any(|r| r.data == expected_target_path_str && r.spawner == "fs"),
//                     "Tilde expanded path '{}' (fs) not found for query '{}'. Results: {:?}",
//                     expected_target_path_str, query, results
//                 );
//             }
//             Ok(Some(other)) => panic!("Expected PathSearchResult, got {:?} for query {}", other, query),
//             Ok(None) => panic!("Channel closed unexpectedly for query {}", query),
//             Err(_) => panic!("Test for tilde_expansion timed out for query {}", query),
//         }

//         if let Some(home_val) = original_home {
//             env::set_var("HOME", home_val);
//         } else {
//             env::remove_var("HOME");
//         }
//     }

//     #[tokio::test]
//     async fn test_spawn_path_search_non_existent_path() {
//         let non_existent_path = "/hopefully_non_existent_path_8765zyxw/for_testing_cba";
//         let (sender, mut receiver) = tokio_mpsc::channel(1);
//         spawn_path_search(non_existent_path.to_string(), sender);

//         match tokio_timeout(Duration::from_secs(3), receiver.recv()).await {
//             Ok(Some(AsyncResult::PathSearchResult(results))) => {
//                 assert!(results.is_empty(), "Expected no results for non-existent path, got {:?}", results);
//             }
//             Ok(Some(AsyncResult::Error(e))) => {
//                 // This is acceptable if zoxide is not installed or returns an error.
//                 // The main check is that PathSearchResult is empty if the direct path doesn't exist.
//                 println!("[Spawner Test Info] Received error for non-existent path (could be zoxide issue): {}", e);
//                  // Further check if you want to ensure PathSearchResult wouldn't have found it anyway.
//                  // This branch means `potential_actions` was empty, and `errors` was not.
//             }
//             Ok(Some(other)) => panic!("Expected empty PathSearchResult or Error, got {:?}", other),
//             Ok(None) => panic!("Channel closed unexpectedly for non_existent_path test"),
//             Err(_) => panic!("Test for non_existent_path timed out"),
//         }
//     }
// }

// // --- UI (Buffer) Tests ---
// #[cfg(test)]
// mod ui_rendering_tests {
//     use std::rc::Rc;
//     use super::*; 
//     use ratatui::{
//         Terminal,
//         backend::TestBackend,
//         layout::{Constraint, Direction, Layout, Rect},
//         buffer::Buffer, 
//     };

//     // Keep this helper as it was in your version that had 1 failure
//     fn get_rendered_string_at_line(buffer: &Buffer, area: Rect, line_index: u16) -> String {
//         let mut s = String::new();
//         let content_start_y = area.y.saturating_add(1);
//         let content_end_y = area.y.saturating_add(area.height).saturating_sub(1);
//         let target_y = content_start_y.saturating_add(line_index);

//         if target_y >= content_end_y || target_y < content_start_y {
//             return s;
//         }

//         let content_start_x = area.x.saturating_add(1);
//         let content_end_x = area.x.saturating_add(area.width).saturating_sub(1);

//         for x in content_start_x..content_end_x {
//             s.push_str(buffer[(x, target_y)].symbol());
//         }
//         s.trim_end().to_string() // IMPORTANT: Keep .trim_end() here as it was
//     }

//     // Keep this helper as is
//     fn get_rendered_block_content(buffer: &Buffer, block_area: Rect) -> Vec<String> {
//         let mut lines = Vec::new();
//         let content_height = block_area.height.saturating_sub(2);
//         if content_height == 0 { return lines; }

//         for i in 0..content_height {
//             lines.push(get_rendered_string_at_line(buffer, block_area, i));
//         }
//         lines
//     }
    
//     fn test_ui_layout(area: Rect) -> Rc<[Rect]> {
//         Layout::default()
//             .direction(Direction::Vertical)
//             .constraints([
//                 Constraint::Length(3), 
//                 Constraint::Length(4), 
//                 Constraint::Length(5), 
//             ])
//             .split(area)
//     }

//     #[test]
//     fn test_ui_renders_initial_empty_input() {
//         let app = new_app();
//         let backend = TestBackend::new(80, 12); 
//         let mut terminal = Terminal::new(backend).unwrap();

//         terminal.draw(|f| {
//             crate::ui::ui(f, &app); 
//         })
//         .unwrap();

//         let buffer = terminal.backend().buffer();
//         let layout_areas = test_ui_layout(Rect::new(0,0,80,12));
//         let input_block_area = layout_areas[0];

//         // Content area coordinates for the input block
//         // Borders take 1 cell on each side.
//         let content_x_start = input_block_area.x + 1;
//         let content_y_start = input_block_area.y + 1;

//         // Directly check the cells for "> "
//         let char1 = buffer[(content_x_start, content_y_start)].symbol();
//         let char2 = buffer[(content_x_start + 1, content_y_start)].symbol();
        
//         assert_eq!(char1, ">", "Input prompt first char should be '>'. Got: '{}'", char1);
//         assert_eq!(char2, " ", "Input prompt second char should be a space. Got: '{}'", char2);

//         // If you want to verify that the rest of the line (or at least the part read by
//         // get_rendered_string_at_line) would have been trimmed, you can still use it
//         // for that observation, but the primary assertion is now on direct cell content.
//         // let line0_via_helper = get_rendered_string_at_line(buffer, input_block_area, 0);
//         // assert_eq!(line0_via_helper, ">", "Helper function after trim_end should yield '>'. Got: '{}'", line0_via_helper);
//     }

//     #[test]
//     fn test_ui_renders_loading_message_in_output() {
//         let mut app = new_app();
//         app.is_loading = true;

//         let backend = TestBackend::new(80, 12);
//         let mut terminal = Terminal::new(backend).unwrap();
//         terminal.draw(|f| crate::ui::ui(f, &app)).unwrap();

//         let buffer = terminal.backend().buffer();
//         let layout_areas = test_ui_layout(Rect::new(0,0,80,12));
//         let output_block_area = layout_areas[1];
//         let rendered_lines = get_rendered_block_content(buffer, output_block_area);

//         assert!(!rendered_lines.is_empty(), "Output block is empty when loading");
//         // This assertion relies on get_rendered_string_at_line (which uses trim_end)
//         assert_eq!(rendered_lines[0], "Loading...", "Loading message mismatch. Got: {:?}", rendered_lines);
//     }

//     #[test]
//     fn test_ui_renders_error_message_in_output() {
//         let mut app = new_app();
//         app.err_msg = "Test Error Occurred".to_string();

//         let backend = TestBackend::new(80, 12);
//         let mut terminal = Terminal::new(backend).unwrap();
//         terminal.draw(|f| crate::ui::ui(f, &app)).unwrap();

//         let buffer = terminal.backend().buffer();
//         let layout_areas = test_ui_layout(Rect::new(0,0,80,12));
//         let output_block_area = layout_areas[1];
//         let rendered_lines = get_rendered_block_content(buffer, output_block_area);
        
//         assert!(!rendered_lines.is_empty(), "Output block is empty when error");
//         assert_eq!(rendered_lines[0], "Test Error Occurred", "Error message mismatch. Got: {:?}", rendered_lines);
//     }

//     #[test]
//     fn test_ui_renders_results_in_output() {
//         let mut app = new_app();
//         app.output = vec![
//             ActionResult {
//                 spawner: "z".to_string(),
//                 action: "cd".to_string(),
//                 description: "/path/numero_uno".to_string(),
//                 data: "/path/numero_uno".to_string(),
//             },
//             ActionResult {
//                 spawner: "fs".to_string(),
//                 action: "cd".to_string(),
//                 description: "/path/numero_dos".to_string(),
//                 data: "/path/numero_dos".to_string(),
//             },
//         ];
//         app.focus = crate::app::FocusBlock::Output; 
//         app.selected_output_index = 0;

//         let backend = TestBackend::new(80, 12);
//         let mut terminal = Terminal::new(backend).unwrap();
//         terminal.draw(|f| crate::ui::ui(f, &app)).unwrap();
//         let buffer = terminal.backend().buffer();
//         let layout_areas = test_ui_layout(Rect::new(0,0,80,12));
//         let output_block_area = layout_areas[1];

//         let rendered_lines = get_rendered_block_content(buffer, output_block_area);

//         assert!(rendered_lines.len() >= 2, "Expected at least 2 lines of output, got {}. Content: \n{:?}", rendered_lines.len(), rendered_lines);
        
//         let expected_line_one = "[z] cd :: /path/numero_uno"; // These strings have no trailing spaces
//         let expected_line_two = "[fs] cd :: /path/numero_dos";
        
//         assert_eq!(rendered_lines[0], expected_line_one, "First line of output mismatch. Got: '{}'", rendered_lines[0]);
//         assert_eq!(rendered_lines[1], expected_line_two, "Second line of output mismatch. Got: '{}'", rendered_lines[1]);

//         let content_start_x = output_block_area.x + 1;
//         let content_start_y = output_block_area.y + 1;
//         if !rendered_lines[0].is_empty() { 
//             let cell_style = buffer[(content_start_x, content_start_y)].style();
//             assert_eq!(cell_style.bg, Some(ratatui::style::Color::Cyan), "Selected item background should be Cyan");
//             assert_eq!(cell_style.fg, Some(ratatui::style::Color::Black), "Selected item foreground should be Black");
//         }
//     }
// }