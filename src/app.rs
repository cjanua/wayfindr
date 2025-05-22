// src/app.rs
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::{
    config::get_config,
    providers::ProviderManager,
    services::{execution::ExecutionService, usage},
    types::{ActionResult, AppResult, SearchMessage},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusState {
    Input,
    Results,
}

pub struct App {
    // UI State
    pub input: String,
    pub results: Vec<ActionResult>,
    pub selected_index: usize,
    pub focus: FocusState,
    pub is_loading: bool,
    pub error_message: Option<String>,

    // History
    pub history: Vec<String>,
    pub history_index: Option<usize>,

    // Services
    provider_manager: ProviderManager,
    execution_service: ExecutionService,

    // Control
    should_exit: bool,
}

impl App {
    pub async fn new() -> AppResult<Self> {
        let config = get_config();
        let mut provider_manager = ProviderManager::default();
        provider_manager.configure_all(config);

        let execution_service = ExecutionService::new();

        let mut app = Self {
            input: String::new(),
            results: Vec::new(),
            selected_index: 0,
            focus: FocusState::Input,
            is_loading: false,
            error_message: None,
            history: Vec::new(),
            history_index: None,
            provider_manager,
            execution_service,
            should_exit: false,
        };

        // Load initial results (top used apps)
        app.load_initial_results().await;

        Ok(app)
    }

    // Update the load_initial_results method to show actual top used apps
    async fn load_initial_results(&mut self) {
        let top_used = usage::get_top_used(5);
        if !top_used.is_empty() {
            // Get the actual app names and try to find them in the applications provider
            let mut initial_results = Vec::new();

            for app_id in top_used {
                // Try to reconstruct ActionResult from usage data
                // This is a simplified approach - you might want to cache app data
                if let Some(result) = self.get_app_by_id(&app_id).await {
                    initial_results.push(result);
                }
            }

            self.results = initial_results;
            self.selected_index = 0;
        } else {
            // If no usage data, show some default apps or empty
            self.results = Vec::new();
        }
    }

    // Add helper method to check if query is AI-related
    fn is_ai_query(&self, query: &str) -> bool {
        let config = get_config();
        query.starts_with(&config.search.ai_prefix) || query.starts_with("ask:")
    }

    // Helper method to get app by ID (simplified - you might want to improve this)
    async fn get_app_by_id(&self, app_id: &str) -> Option<ActionResult> {
        // This is a simplified implementation
        // You might want to cache application data or query the provider directly

        // Extract app name from ID (assuming format like "app_<hash>")
        if app_id.starts_with("app_") {
            // For now, create a simple launch action
            // In a real implementation, you'd want to store more app metadata
            let app_name = app_id.strip_prefix("app_").unwrap_or(app_id);

            Some(ActionResult::new_launch(
                app_id.to_string(),
                "applications",
                format!("App: {}", app_name),
                app_name.to_string(),
                false,
            ))
        } else {
            None
        }
    }

    // Update the run method to load initial results right after starting
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        search_tx: mpsc::Sender<SearchMessage>,
        mut search_rx: mpsc::Receiver<SearchMessage>,
    ) -> AppResult<()> {
        // Load initial results (top used apps) instead of empty search
        self.load_initial_results().await;

        loop {
            if self.should_exit {
                break;
            }

            // Draw UI
            terminal.draw(|frame| crate::ui::render(frame, self))?;

            // Handle events with timeout
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        self.handle_key_event(key_event, &search_tx).await?;
                    }
                }
            }

            // Handle search messages
            while let Ok(message) = search_rx.try_recv() {
                self.handle_search_message(message);
            }
        }

        Ok(())
    }

    async fn handle_key_event(
        &mut self,
        event: KeyEvent,
        search_tx: &mpsc::Sender<SearchMessage>,
    ) -> AppResult<()> {
        match event.code {
            KeyCode::Esc => self.should_exit = true,

            KeyCode::Enter => match self.focus {
                FocusState::Input => {
                    self.handle_input_enter(search_tx).await?;
                }
                FocusState::Results => {
                    self.handle_result_selection().await?;
                }
            },

            KeyCode::Tab => {
                self.cycle_focus();
            }

            KeyCode::Up => match self.focus {
                FocusState::Input => self.navigate_history(-1),
                FocusState::Results => self.navigate_results(-1),
            },

            KeyCode::Down => match self.focus {
                FocusState::Input => self.navigate_history(1),
                FocusState::Results => self.navigate_results(1),
            },

            KeyCode::Char(c) => {
                if self.focus == FocusState::Input {
                    self.input.push(c);
                    self.history_index = None;

                    // Only trigger live search for NON-AI queries
                    if get_config().search.enable_live_search && !self.is_ai_query(&self.input) {
                        let input = self.input.clone();
                        self.perform_search(&input, search_tx).await;
                    }
                }
            }

            KeyCode::Backspace => {
                if self.focus == FocusState::Input && !self.input.is_empty() {
                    self.input.pop();
                    self.history_index = None;

                    // Only trigger live search for NON-AI queries
                    if get_config().search.enable_live_search && !self.is_ai_query(&self.input) {
                        let input = self.input.clone();
                        self.perform_search(&input, search_tx).await;
                    } else if self.input.is_empty() {
                        // Show top apps when input becomes empty
                        self.load_initial_results().await;
                    }
                }
            }

            _ => {}
        }

        Ok(())
    }

    async fn handle_input_enter(
        &mut self,
        search_tx: &mpsc::Sender<SearchMessage>,
    ) -> AppResult<()> {
        let query = self.input.trim().to_string();

        if !query.is_empty() {
            // Add to history
            self.add_to_history(query.clone());

            // Clear input and perform search
            self.input.clear();
            self.history_index = None;
            self.perform_search(&query, search_tx).await;

            // Switch focus to results if we have any
            if !self.results.is_empty() {
                self.focus = FocusState::Results;
                self.selected_index = 0;
            }
        }

        Ok(())
    }

    async fn handle_result_selection(&mut self) -> AppResult<()> {
        if let Some(result) = self.results.get(self.selected_index) {
            // Record usage
            usage::record_usage(&result.id);

            // Execute the action
            match self.execution_service.execute(result).await {
                Ok(should_exit) => {
                    if should_exit {
                        self.should_exit = true;
                    } else {
                        // Clear results and return to input
                        self.results.clear();
                        self.focus = FocusState::Input;
                        self.selected_index = 0;
                        self.error_message = None;
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Execution failed: {}", e));
                    self.focus = FocusState::Input;
                }
            }
        }

        Ok(())
    }

    async fn perform_search(&mut self, query: &str, search_tx: &mpsc::Sender<SearchMessage>) {
        self.is_loading = true;
        self.error_message = None;

        let search_tx = search_tx.clone();
        let query = query.to_string();
        let provider_manager = self.provider_manager.clone();

        tokio::spawn(async move {
            let results = provider_manager.search_all(&query).await;

            let message = if results.is_empty() {
                SearchMessage::Results(Vec::new())
            } else {
                SearchMessage::Results(results)
            };

            let _ = search_tx.send(message).await;
        });
    }

    fn handle_search_message(&mut self, message: SearchMessage) {
        match message {
            SearchMessage::Results(scored_results) => {
                self.results = scored_results.into_iter().map(|sr| sr.result).collect();
                self.selected_index = 0;
                self.is_loading = false;
            }
            SearchMessage::Error(error) => {
                self.error_message = Some(error);
                self.is_loading = false;
            }
            SearchMessage::Loading(loading) => {
                self.is_loading = loading;
            }
            _ => {}
        }
    }

    fn cycle_focus(&mut self) {
        match self.focus {
            FocusState::Input => {
                if !self.results.is_empty() {
                    self.focus = FocusState::Results;
                    self.selected_index = 0;
                }
            }
            FocusState::Results => {
                self.focus = FocusState::Input;
            }
        }
    }

    fn navigate_history(&mut self, direction: i32) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => {
                if direction > 0 {
                    0
                } else {
                    self.history.len() - 1
                }
            }
            Some(current) => {
                if direction > 0 {
                    (current + 1).min(self.history.len() - 1)
                } else {
                    current.saturating_sub(1)
                }
            }
        };

        self.history_index = Some(new_index);
        self.input = self.history[new_index].clone();
    }

    fn navigate_results(&mut self, direction: i32) {
        if self.results.is_empty() {
            return;
        }

        if direction > 0 {
            self.selected_index = (self.selected_index + 1).min(self.results.len() - 1);
        } else {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    fn add_to_history(&mut self, query: String) {
        let config = get_config();

        // Don't add if it's the same as the last entry
        if self.history.first() == Some(&query) {
            return;
        }

        // Add to front of history
        self.history.insert(0, query);

        // Trim to configured limit
        if self.history.len() > config.general.history_limit {
            self.history.truncate(config.general.history_limit);
        }
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }
}

// Make ProviderManager cloneable for async operations
impl Clone for ProviderManager {
    fn clone(&self) -> Self {
        // This is a simplified clone - in practice you'd want proper cloning
        ProviderManager::default()
    }
}
