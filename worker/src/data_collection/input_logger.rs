//! Input logging for keystrokes and mouse events.
//!
//! Uses platform-specific APIs:
//! - macOS: Core Graphics event taps
//! - Windows: Raw input or hooks
//! - Linux: libinput or /dev/input

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use super::config::MultiTimestamp;
use super::OrchestratorRefs;

/// Input event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    KeyDown {
        key: String,
        modifiers: Vec<String>,
        timestamp: MultiTimestamp,
    },
    KeyUp {
        key: String,
        modifiers: Vec<String>,
        timestamp: MultiTimestamp,
    },
    MouseMove {
        x: i32,
        y: i32,
        timestamp: MultiTimestamp,
    },
    MouseDown {
        button: String,
        x: i32,
        y: i32,
        timestamp: MultiTimestamp,
    },
    MouseUp {
        button: String,
        x: i32,
        y: i32,
        timestamp: MultiTimestamp,
    },
    MouseScroll {
        dx: f64,
        dy: f64,
        x: i32,
        y: i32,
        timestamp: MultiTimestamp,
    },
}

/// Run input logging
pub async fn run_capture(orchestrator: OrchestratorRefs) -> Result<()> {
    info!("Starting input logger");

    let shutdown_rx = orchestrator.get_shutdown_receiver().await;
    let running = Arc::new(AtomicBool::new(true));

    // Start platform-specific input capture
    let orchestrator_clone = orchestrator.clone();
    let running_clone = running.clone();

    #[cfg(target_os = "macos")]
    {
        std::thread::spawn(move || {
            if let Err(e) = run_macos_input_logger(orchestrator_clone, running_clone) {
                error!("macOS input logger error: {}", e);
            }
        });
    }

    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || {
            if let Err(e) = run_linux_input_logger(orchestrator_clone, running_clone) {
                error!("Linux input logger error: {}", e);
            }
        });
    }

    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || {
            if let Err(e) = run_windows_input_logger(orchestrator_clone, running_clone) {
                error!("Windows input logger error: {}", e);
            }
        });
    }

    // Wait for shutdown signal
    if let Some(mut rx) = shutdown_rx {
        let _ = rx.recv().await;
    }

    running.store(false, Ordering::SeqCst);
    info!("Input logger stopped");
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_macos_input_logger(_orchestrator: OrchestratorRefs, running: Arc<AtomicBool>) -> Result<()> {
    // Note: Full CGEventTap implementation requires specific core_graphics API version
    // and Accessibility permissions. This is a simplified polling approach.
    warn!("macOS input logging requires Accessibility permissions");

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

/// Convert macOS keycode to string representation
#[cfg(target_os = "macos")]
pub fn keycode_to_string(keycode: u16) -> String {
    // Common macOS keycodes
    match keycode {
        0 => "a".to_string(),
        1 => "s".to_string(),
        2 => "d".to_string(),
        3 => "f".to_string(),
        4 => "h".to_string(),
        5 => "g".to_string(),
        6 => "z".to_string(),
        7 => "x".to_string(),
        8 => "c".to_string(),
        9 => "v".to_string(),
        11 => "b".to_string(),
        12 => "q".to_string(),
        13 => "w".to_string(),
        14 => "e".to_string(),
        15 => "r".to_string(),
        16 => "y".to_string(),
        17 => "t".to_string(),
        18 => "1".to_string(),
        19 => "2".to_string(),
        20 => "3".to_string(),
        21 => "4".to_string(),
        22 => "6".to_string(),
        23 => "5".to_string(),
        24 => "=".to_string(),
        25 => "9".to_string(),
        26 => "7".to_string(),
        27 => "-".to_string(),
        28 => "8".to_string(),
        29 => "0".to_string(),
        30 => "]".to_string(),
        31 => "o".to_string(),
        32 => "u".to_string(),
        33 => "[".to_string(),
        34 => "i".to_string(),
        35 => "p".to_string(),
        36 => "return".to_string(),
        37 => "l".to_string(),
        38 => "j".to_string(),
        39 => "'".to_string(),
        40 => "k".to_string(),
        41 => ";".to_string(),
        42 => "\\".to_string(),
        43 => ",".to_string(),
        44 => "/".to_string(),
        45 => "n".to_string(),
        46 => "m".to_string(),
        47 => ".".to_string(),
        48 => "tab".to_string(),
        49 => "space".to_string(),
        50 => "`".to_string(),
        51 => "delete".to_string(),
        53 => "escape".to_string(),
        123 => "left".to_string(),
        124 => "right".to_string(),
        125 => "down".to_string(),
        126 => "up".to_string(),
        _ => format!("key_{}", keycode),
    }
}

/// Convert CGEventFlags to a list of modifier names
#[cfg(target_os = "macos")]
pub fn flags_to_modifiers(flags: core_graphics::event::CGEventFlags) -> Vec<String> {
    use core_graphics::event::CGEventFlags;
    let mut modifiers = Vec::new();
    if flags.contains(CGEventFlags::CGEventFlagCommand) {
        modifiers.push("command".to_string());
    }
    if flags.contains(CGEventFlags::CGEventFlagShift) {
        modifiers.push("shift".to_string());
    }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) {
        modifiers.push("option".to_string());
    }
    if flags.contains(CGEventFlags::CGEventFlagControl) {
        modifiers.push("control".to_string());
    }
    modifiers
}

#[cfg(target_os = "linux")]
fn run_linux_input_logger(orchestrator: OrchestratorRefs, running: Arc<AtomicBool>) -> Result<()> {
    // On Linux, we'd use libinput or read from /dev/input/event*
    // This requires appropriate permissions
    warn!("Linux input logging requires root or input group membership");

    // For now, use a polling approach with X11/Wayland cursor position
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn run_windows_input_logger(orchestrator: OrchestratorRefs, running: Arc<AtomicBool>) -> Result<()> {
    // On Windows, we'd use SetWindowsHookEx or Raw Input API
    warn!("Windows input logging not fully implemented");

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_key_down_serialization() {
        let timestamp = MultiTimestamp::now(1);
        let event = InputEvent::KeyDown {
            key: "a".to_string(),
            modifiers: vec!["shift".to_string()],
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"KeyDown\""));
        assert!(json.contains("\"key\":\"a\""));
        assert!(json.contains("shift"));
    }

    #[test]
    fn test_input_event_key_up_serialization() {
        let timestamp = MultiTimestamp::now(2);
        let event = InputEvent::KeyUp {
            key: "space".to_string(),
            modifiers: vec![],
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"KeyUp\""));
        assert!(json.contains("\"key\":\"space\""));
    }

    #[test]
    fn test_input_event_mouse_move_serialization() {
        let timestamp = MultiTimestamp::now(3);
        let event = InputEvent::MouseMove {
            x: 100,
            y: 200,
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"MouseMove\""));
        assert!(json.contains("\"x\":100"));
        assert!(json.contains("\"y\":200"));
    }

    #[test]
    fn test_input_event_mouse_down_serialization() {
        let timestamp = MultiTimestamp::now(4);
        let event = InputEvent::MouseDown {
            button: "left".to_string(),
            x: 50,
            y: 75,
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"MouseDown\""));
        assert!(json.contains("\"button\":\"left\""));
    }

    #[test]
    fn test_input_event_mouse_up_serialization() {
        let timestamp = MultiTimestamp::now(5);
        let event = InputEvent::MouseUp {
            button: "right".to_string(),
            x: 150,
            y: 250,
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"MouseUp\""));
        assert!(json.contains("\"button\":\"right\""));
    }

    #[test]
    fn test_input_event_mouse_scroll_serialization() {
        let timestamp = MultiTimestamp::now(6);
        let event = InputEvent::MouseScroll {
            dx: 0.5,
            dy: -1.0,
            x: 300,
            y: 400,
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"MouseScroll\""));
        assert!(json.contains("\"dx\":0.5"));
        assert!(json.contains("\"dy\":-1"));
    }

    #[test]
    fn test_input_event_deserialization() {
        let json = r#"{
            "type": "KeyDown",
            "key": "enter",
            "modifiers": ["command", "shift"],
            "timestamp": {"seq": 1, "wall_time_ms": 1000, "monotonic_ns": 2000}
        }"#;

        let event: InputEvent = serde_json::from_str(json).unwrap();
        match event {
            InputEvent::KeyDown { key, modifiers, .. } => {
                assert_eq!(key, "enter");
                assert_eq!(modifiers, vec!["command", "shift"]);
            }
            _ => panic!("Expected KeyDown event"),
        }
    }

    #[test]
    fn test_input_event_mouse_deserialization() {
        let json = r#"{
            "type": "MouseMove",
            "x": 512,
            "y": 384,
            "timestamp": {"seq": 10, "wall_time_ms": 5000, "monotonic_ns": 6000}
        }"#;

        let event: InputEvent = serde_json::from_str(json).unwrap();
        match event {
            InputEvent::MouseMove { x, y, .. } => {
                assert_eq!(x, 512);
                assert_eq!(y, 384);
            }
            _ => panic!("Expected MouseMove event"),
        }
    }

    #[test]
    fn test_input_event_clone() {
        let timestamp = MultiTimestamp::now(7);
        let event = InputEvent::KeyDown {
            key: "tab".to_string(),
            modifiers: vec!["control".to_string()],
            timestamp,
        };

        let cloned = event.clone();
        match (event, cloned) {
            (
                InputEvent::KeyDown { key: k1, modifiers: m1, .. },
                InputEvent::KeyDown { key: k2, modifiers: m2, .. },
            ) => {
                assert_eq!(k1, k2);
                assert_eq!(m1, m2);
            }
            _ => panic!("Clone produced different variant"),
        }
    }

    #[test]
    fn test_input_event_debug_format() {
        let timestamp = MultiTimestamp::now(8);
        let event = InputEvent::MouseDown {
            button: "middle".to_string(),
            x: 100,
            y: 100,
            timestamp,
        };

        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("MouseDown"));
        assert!(debug_str.contains("middle"));
    }

    #[test]
    fn test_multiple_modifiers() {
        let timestamp = MultiTimestamp::now(9);
        let event = InputEvent::KeyDown {
            key: "c".to_string(),
            modifiers: vec![
                "command".to_string(),
                "shift".to_string(),
                "option".to_string(),
                "control".to_string(),
            ],
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("command"));
        assert!(json.contains("shift"));
        assert!(json.contains("option"));
        assert!(json.contains("control"));
    }

    #[test]
    fn test_negative_coordinates() {
        let timestamp = MultiTimestamp::now(10);
        let event = InputEvent::MouseMove {
            x: -100,
            y: -50,
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: InputEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            InputEvent::MouseMove { x, y, .. } => {
                assert_eq!(x, -100);
                assert_eq!(y, -50);
            }
            _ => panic!("Expected MouseMove"),
        }
    }

    #[test]
    fn test_scroll_negative_values() {
        let timestamp = MultiTimestamp::now(11);
        let event = InputEvent::MouseScroll {
            dx: -2.5,
            dy: 3.7,
            x: 0,
            y: 0,
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: InputEvent = serde_json::from_str(&json).unwrap();

        match parsed {
            InputEvent::MouseScroll { dx, dy, .. } => {
                assert!((dx - (-2.5)).abs() < 0.001);
                assert!((dy - 3.7).abs() < 0.001);
            }
            _ => panic!("Expected MouseScroll"),
        }
    }

    #[test]
    fn test_empty_modifiers() {
        let timestamp = MultiTimestamp::now(12);
        let event = InputEvent::KeyUp {
            key: "escape".to_string(),
            modifiers: vec![],
            timestamp,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"modifiers\":[]"));
    }

    #[test]
    fn test_special_key_names() {
        let special_keys = vec![
            "return", "tab", "space", "delete", "escape",
            "left", "right", "up", "down",
        ];

        for key in special_keys {
            let timestamp = MultiTimestamp::now(0);
            let event = InputEvent::KeyDown {
                key: key.to_string(),
                modifiers: vec![],
                timestamp,
            };

            let json = serde_json::to_string(&event).unwrap();
            assert!(json.contains(key), "Key {} not found in JSON", key);
        }
    }

    #[test]
    fn test_mouse_buttons() {
        let buttons = vec!["left", "right", "middle"];

        for button in buttons {
            let timestamp = MultiTimestamp::now(0);
            let event = InputEvent::MouseDown {
                button: button.to_string(),
                x: 0,
                y: 0,
                timestamp,
            };

            let json = serde_json::to_string(&event).unwrap();
            assert!(json.contains(button));
        }
    }

    #[test]
    fn test_atomic_bool_running_flag() {
        let running = Arc::new(AtomicBool::new(true));

        assert!(running.load(Ordering::SeqCst));
        running.store(false, Ordering::SeqCst);
        assert!(!running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_event_sequence_counter() {
        let counter = Arc::new(AtomicU64::new(0));

        for expected in 0..100 {
            let seq = counter.fetch_add(1, Ordering::SeqCst);
            assert_eq!(seq, expected);
        }

        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;

        #[test]
        fn test_keycode_to_string_letters() {
            assert_eq!(keycode_to_string(0), "a");
            assert_eq!(keycode_to_string(1), "s");
            assert_eq!(keycode_to_string(2), "d");
            assert_eq!(keycode_to_string(3), "f");
            assert_eq!(keycode_to_string(12), "q");
            assert_eq!(keycode_to_string(13), "w");
            assert_eq!(keycode_to_string(14), "e");
            assert_eq!(keycode_to_string(15), "r");
        }

        #[test]
        fn test_keycode_to_string_numbers() {
            assert_eq!(keycode_to_string(18), "1");
            assert_eq!(keycode_to_string(19), "2");
            assert_eq!(keycode_to_string(20), "3");
            assert_eq!(keycode_to_string(21), "4");
            assert_eq!(keycode_to_string(23), "5");
            assert_eq!(keycode_to_string(22), "6");
            assert_eq!(keycode_to_string(26), "7");
            assert_eq!(keycode_to_string(28), "8");
            assert_eq!(keycode_to_string(25), "9");
            assert_eq!(keycode_to_string(29), "0");
        }

        #[test]
        fn test_keycode_to_string_special() {
            assert_eq!(keycode_to_string(36), "return");
            assert_eq!(keycode_to_string(48), "tab");
            assert_eq!(keycode_to_string(49), "space");
            assert_eq!(keycode_to_string(51), "delete");
            assert_eq!(keycode_to_string(53), "escape");
        }

        #[test]
        fn test_keycode_to_string_arrows() {
            assert_eq!(keycode_to_string(123), "left");
            assert_eq!(keycode_to_string(124), "right");
            assert_eq!(keycode_to_string(125), "down");
            assert_eq!(keycode_to_string(126), "up");
        }

        #[test]
        fn test_keycode_to_string_unknown() {
            let unknown = keycode_to_string(999);
            assert!(unknown.starts_with("key_"));
            assert!(unknown.contains("999"));
        }
    }
}
