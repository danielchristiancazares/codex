//! Optional cell export for independently reviewable images of real widget fixtures.

use ratatui::buffer::Buffer;
use ratatui::style::Modifier;
use serde_json::json;
use std::path::PathBuf;

pub(crate) fn export_visual_review_buffer(name: &str, buffer: &Buffer) {
    let Some(directory) = std::env::var_os("CODEX_TUI_REVIEW_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create review directory");
    let cells = buffer
        .content
        .iter()
        .map(|cell| {
            json!({
                "text": cell.symbol(),
                "fg": cell.fg.to_string(),
                "bg": cell.bg.to_string(),
                "bold": cell.modifier.contains(Modifier::BOLD),
                "dim": cell.modifier.contains(Modifier::DIM),
                "reverse": cell.modifier.contains(Modifier::REVERSED),
                "italic": cell.modifier.contains(Modifier::ITALIC),
                "underline": cell.modifier.contains(Modifier::UNDERLINED),
            })
        })
        .collect::<Vec<_>>();
    let fg = crate::terminal_palette::default_fg().unwrap_or((220, 220, 216));
    let bg = crate::terminal_palette::default_bg().unwrap_or((32, 32, 32));
    let data = json!({"width": buffer.area.width, "height": buffer.area.height, "fg": fg, "bg": bg, "cells": cells});
    std::fs::write(
        directory.join(format!("{name}.json")),
        serde_json::to_vec(&data).expect("serialize review cells"),
    )
    .expect("export review cells");
}
