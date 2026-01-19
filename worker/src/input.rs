//! Input simulation for keyboard and mouse.

use anyhow::{anyhow, Result};
use enigo::{Direction, Enigo, Key, Keyboard, Mouse, Settings};
use serde_json::{json, Value};
use tracing::debug;

/// Press a key with optional modifiers.
pub async fn key_press(key: &str, modifiers: &[String]) -> Result<Value> {
    debug!("Key press: {} with modifiers: {:?}", key, modifiers);

    let key_str = key.to_string();
    let mods: Vec<String> = modifiers.to_vec();

    tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("Failed to create Enigo: {:?}", e))?;

        // Press modifiers
        for modifier in &mods {
            let key = match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => Key::Control,
                "alt" | "option" => Key::Alt,
                "shift" => Key::Shift,
                "meta" | "cmd" | "command" | "win" | "super" => Key::Meta,
                _ => continue,
            };
            enigo.key(key, Direction::Press)
                .map_err(|e| anyhow!("Key press failed: {:?}", e))?;
        }

        // Press the main key
        let key = parse_key(&key_str);
        enigo.key(key, Direction::Click)
            .map_err(|e| anyhow!("Key click failed: {:?}", e))?;

        // Release modifiers in reverse order
        for modifier in mods.iter().rev() {
            let key = match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => Key::Control,
                "alt" | "option" => Key::Alt,
                "shift" => Key::Shift,
                "meta" | "cmd" | "command" | "win" | "super" => Key::Meta,
                _ => continue,
            };
            enigo.key(key, Direction::Release)
                .map_err(|e| anyhow!("Key release failed: {:?}", e))?;
        }

        Ok(json!({"status": "ok"}))
    })
    .await?
}

/// Type a string of text.
pub async fn key_type(text: &str) -> Result<Value> {
    debug!("Key type: {}", text);

    let text = text.to_string();

    tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("Failed to create Enigo: {:?}", e))?;

        enigo.text(&text)
            .map_err(|e| anyhow!("Text input failed: {:?}", e))?;

        Ok(json!({
            "status": "ok",
            "characters_typed": text.len(),
        }))
    })
    .await?
}

/// Move the mouse cursor.
pub async fn mouse_move(x: i32, y: i32) -> Result<Value> {
    debug!("Mouse move to: ({}, {})", x, y);

    tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("Failed to create Enigo: {:?}", e))?;

        enigo.move_mouse(x, y, enigo::Coordinate::Abs)
            .map_err(|e| anyhow!("Mouse move failed: {:?}", e))?;

        Ok(json!({
            "status": "ok",
            "x": x,
            "y": y,
        }))
    })
    .await?
}

/// Click a mouse button.
pub async fn mouse_click(button: &str) -> Result<Value> {
    debug!("Mouse click: {}", button);

    let button = button.to_string();

    tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow!("Failed to create Enigo: {:?}", e))?;

        let mouse_button = match button.to_lowercase().as_str() {
            "left" => enigo::Button::Left,
            "right" => enigo::Button::Right,
            "middle" => enigo::Button::Middle,
            _ => enigo::Button::Left,
        };

        enigo.button(mouse_button, Direction::Click)
            .map_err(|e| anyhow!("Mouse click failed: {:?}", e))?;

        Ok(json!({
            "status": "ok",
            "button": button,
        }))
    })
    .await?
}

fn parse_key(key: &str) -> Key {
    match key.to_lowercase().as_str() {
        // Special keys
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "escape" | "esc" => Key::Escape,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,

        // Function keys
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,

        // Modifiers (as standalone keys)
        "ctrl" | "control" => Key::Control,
        "alt" | "option" => Key::Alt,
        "shift" => Key::Shift,
        "meta" | "cmd" | "command" | "win" | "super" => Key::Meta,

        // Default: treat as unicode character
        _ => {
            if key.len() == 1 {
                Key::Unicode(key.chars().next().unwrap())
            } else {
                Key::Return // Fallback
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_special() {
        assert!(matches!(parse_key("enter"), Key::Return));
        assert!(matches!(parse_key("tab"), Key::Tab));
        assert!(matches!(parse_key("escape"), Key::Escape));
    }

    #[test]
    fn test_parse_key_unicode() {
        assert!(matches!(parse_key("a"), Key::Unicode('a')));
        assert!(matches!(parse_key("1"), Key::Unicode('1')));
    }
}
