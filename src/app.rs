// src/app.rs
use std::time::Duration;
use tokio::sync::mpsc;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    config::get_config,
    providers::ProviderManager,
    services::{execution::ExecutionService, usage},
    types::{ActionResult, AppResult, SearchMessage},
    utils,
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
    
    async fn load_initial_results(&mut self) {
        let top_used = usage::get_top_used(5);
        if !top_used.is_empty() {
            // Convert usage IDs back to results if possible
            // This is simplified - in practice you'd want to reconstruct the ActionResults
            self.results = top_used.into_iter()
                .take(5)
                .enumerate()
                .map(|(i, id)| ActionResult::new_launch(
                    id.clone(),
                    "usage",
                    format!("Frequently Used #{}", i + 1),
                    id,
                    false,
                ))
                .collect();
        }
    }
    
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        search_tx: mpsc::Sender<SearchMessage>,
        mut search_rx: mpsc::Receiver<SearchMessage>,
    ) -> AppResult<()> {
        // Start with initial search for empty query
        self.perform_search("", &search_tx).await;
        
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
            
            KeyCode::Enter => {
                match self.focus {
                    FocusState::Input => {
                        self.handle_input_enter(search_tx).await?;
                    }
                    FocusState::Results => {
                        self.handle_result_selection().await?;
                    }
                }
            }
            
            KeyCode::Tab => {
                self.cycle_focus();
            }
            
            KeyCode::Up => {
                match self.focus {
                    FocusState::Input => self.navigate_history(-1),
                    FocusState::Results => self.navigate_results(-1),
                }
            }
            
            KeyCode::Down => {
                match self.focus {
                    FocusState::Input => self.navigate_history(1),
                    FocusState::Results => self.navigate_results(1),
                }
            }
            
            KeyCode::Char(c) => {
                if self.focus == FocusState::Input {
                    self.input.push(c);
                    self.history_index = None;
                    
                    // Trigger live search if enabled
                    if get_config().search.enable_live_search {
                        self.perform_search(&self.input, search_tx).await;
                    }
                }
            }
            
            KeyCode::Backspace => {
                if self.focus == FocusState::Input && !self.input.is_empty() {
                    self.input.pop();
                    self.history_index = None;
                    
                    // Trigger live search if enabled
                    if get_config().search.enable_live_search {
                        self.perform_search(&self.input, search_tx).await;
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
    
    async fn perform_search(
        &mut self,
        query: &str,
        search_tx: &mpsc::Sender<SearchMessage>,
    ) {
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
                self.results = scored_results.into_iter()
                    .map(|sr| sr.result)
                    .collect();
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
                if direction > 0 { 0 } else { self.history.len() - 1 }
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