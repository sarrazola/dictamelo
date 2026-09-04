//! Historial local (pequeño) de transcripciones, guardado como JSON.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub text: String,
    pub duration_ms: u64,
    pub provider: String,
    pub model: String,
    pub language: Option<String>,
    pub pasted: bool,
}

pub struct History {
    entries: VecDeque<HistoryEntry>,
    path: PathBuf,
}

impl History {
    pub fn load(path: PathBuf) -> History {
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<VecDeque<HistoryEntry>>(&s).ok())
            .unwrap_or_default();
        History { entries, path }
    }

    /// Inserta al inicio y recorta al máximo indicado.
    pub fn push(&mut self, entry: HistoryEntry, max: usize) -> std::io::Result<()> {
        self.entries.push_front(entry);
        self.entries.truncate(max.max(1));
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> std::io::Result<bool> {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        let removed = self.entries.len() != before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn clear(&mut self) -> std::io::Result<()> {
        self.entries.clear();
        self.save()
    }

    pub fn entries(&self) -> Vec<HistoryEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&self.entries).map_err(std::io::Error::other)?;
        std::fs::write(&self.path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> HistoryEntry {
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            text: text.into(),
            duration_ms: 1200,
            provider: "groq".into(),
            model: "whisper-large-v3-turbo".into(),
            language: Some("es".into()),
            pasted: true,
        }
    }

    #[test]
    fn keeps_only_max_entries_and_persists() {
        let dir = std::env::temp_dir().join(format!("dictado-history-{}", uuid::Uuid::new_v4()));
        let path = dir.join("history.json");
        let mut h = History::load(path.clone());
        for i in 0..5 {
            h.push(entry(&format!("texto {i}")), 3).unwrap();
        }
        assert_eq!(h.entries().len(), 3);
        assert_eq!(h.entries()[0].text, "texto 4");

        let reloaded = History::load(path);
        assert_eq!(reloaded.entries(), h.entries());
        let id = reloaded.entries()[1].id.clone();
        let mut reloaded = reloaded;
        assert!(reloaded.remove(&id).unwrap());
        assert_eq!(reloaded.entries().len(), 2);
        reloaded.clear().unwrap();
        assert!(reloaded.entries().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
