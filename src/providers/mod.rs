// src/providers/mod.rs
use crate::types::{ActionResult, ProviderError};
use async_trait::async_trait;

pub mod ai;
pub mod applications;
pub mod directories;

#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Unique identifier for this provider
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Check if this provider can handle the given query
    fn can_handle(&self, query: &str) -> bool;

    /// Get the priority of this provider (higher = more important)
    fn priority(&self) -> u8 {
        50 // Default priority
    }

    /// Perform the search
    async fn search(&self, query: &str) -> Result<Vec<ScoredResult>, ProviderError>;

    /// Optional: Provider-specific configuration
    fn configure(&mut self, _config: &crate::config::Config) {}
}

#[derive(Debug, Clone)]
pub struct ScoredResult {
    pub result: ActionResult,
    pub score: i32,
    pub provider_id: String,
}

impl ScoredResult {
    pub fn new(result: ActionResult, score: i32, provider_id: String) -> Self {
        Self {
            result,
            score,
            provider_id,
        }
    }
}

/// Manages all search providers
pub struct ProviderManager {
    providers: Vec<Box<dyn SearchProvider>>,
}

impl ProviderManager {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register<P: SearchProvider + 'static>(&mut self, provider: P) {
        self.providers.push(Box::new(provider));
    }

    pub fn configure_all(&mut self, config: &crate::config::Config) {
        for provider in &mut self.providers {
            provider.configure(config);
        }
    }

    pub async fn search_all(&self, query: &str) -> Vec<ScoredResult> {
        let mut all_results = Vec::new();

        // Get results from all applicable providers
        for provider in &self.providers {
            if provider.can_handle(query) {
                match provider.search(query).await {
                    Ok(mut results) => {
                        // Apply provider priority boost
                        let priority_boost = (provider.priority() as i32 - 50) * 10;
                        for result in &mut results {
                            result.score += priority_boost;
                        }
                        all_results.extend(results);
                    }
                    Err(e) => {
                        crate::utils::log_error(&format!(
                            "Provider '{}' failed: {}",
                            provider.id(),
                            e
                        ));
                    }
                }
            }
        }

        // Sort by score (highest first)
        all_results.sort_by(|a, b| b.score.cmp(&a.score));

        // Limit results
        let max_results = crate::config::get_config().general.max_results;
        all_results.truncate(max_results);

        all_results
    }

    pub fn get_provider(&self, id: &str) -> Option<&dyn SearchProvider> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        let mut manager = Self::new();

        // Register default providers
        manager.register(applications::ApplicationProvider::new());
        manager.register(directories::DirectoryProvider::new());
        manager.register(ai::AiProvider::new());

        manager
    }
}
