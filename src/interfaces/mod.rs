// src/interfaces/mod.rs
use crate::app::App;
use crate::types::AppResult;

pub mod tui;
pub mod rofi;

/// Supported UI interfaces
#[derive(Debug, Clone, PartialEq)]
pub enum InterfaceType {
    Tui,
    Rofi,
}

impl std::str::FromStr for InterfaceType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tui" | "terminal" => Ok(InterfaceType::Tui),
            "rofi" => Ok(InterfaceType::Rofi),
            _ => Err(format!("Unknown interface type: {}", s)),
        }
    }
}

/// Run wayfindr with the specified interface
pub async fn run_interface(interface: InterfaceType, app: App) -> AppResult<()> {
    match interface {
        InterfaceType::Tui => {
            tui::run_tui(app).await
        }
        InterfaceType::Rofi => {
            rofi::run_rofi(app).await
        }
    }
}