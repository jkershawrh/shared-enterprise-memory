use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{LedgerEntry, LedgerReadError, LedgerReader};

#[derive(Default, Clone)]
pub struct InMemoryLedgerReader {
    entries: Arc<RwLock<Vec<LedgerEntry>>>,
    call_count: Arc<AtomicUsize>,
}

impl InMemoryLedgerReader {
    pub async fn insert_entry(&self, entry: LedgerEntry) {
        self.entries.write().await.push(entry);
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    pub fn reset_call_count(&self) {
        self.call_count.store(0, Ordering::SeqCst);
    }
}

#[async_trait]
impl LedgerReader for InMemoryLedgerReader {
    async fn get_entries_by_type_paged(
        &self,
        entry_type: &str,
        after_position: i64,
        limit: i64,
    ) -> Result<Vec<LedgerEntry>, LedgerReadError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let guard = self.entries.read().await;
        let mut results: Vec<LedgerEntry> = guard
            .iter()
            .filter(|e| e.entry_type == entry_type && e.chain_position > after_position)
            .cloned()
            .collect();
        results.sort_by_key(|e| e.chain_position);
        results.truncate(limit as usize);
        Ok(results)
    }

    async fn get_entry(&self, entry_id: Uuid) -> Result<LedgerEntry, LedgerReadError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let guard = self.entries.read().await;
        guard
            .iter()
            .find(|e| e.entry_id == entry_id)
            .cloned()
            .ok_or(LedgerReadError::NotFound(entry_id))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn test_entry(entry_type: &str, position: i64) -> LedgerEntry {
        LedgerEntry {
            entry_id: Uuid::new_v4(),
            entry_type: entry_type.to_string(),
            agent_id: "agent-1".to_string(),
            content: b"test".to_vec(),
            content_type: "application/json".to_string(),
            source_id: "test".to_string(),
            correlation_id: None,
            entry_hash: format!("hash-{position}"),
            hash_version: "V3".to_string(),
            chain_position: position,
            written_ts: Utc::now(),
        }
    }

    #[tokio::test]
    async fn reads_entries_by_type_with_cursor() {
        let reader = InMemoryLedgerReader::default();
        reader
            .insert_entry(test_entry("memory.observation.recorded", 1))
            .await;
        reader
            .insert_entry(test_entry("memory.observation.recorded", 2))
            .await;
        reader
            .insert_entry(test_entry("memory.decision.recorded", 1))
            .await;

        let results = reader
            .get_entries_by_type_paged("memory.observation.recorded", 0, 100)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        let after_1 = reader
            .get_entries_by_type_paged("memory.observation.recorded", 1, 100)
            .await
            .unwrap();
        assert_eq!(after_1.len(), 1);
        assert_eq!(after_1[0].chain_position, 2);
    }

    #[tokio::test]
    async fn returns_empty_for_unknown_type() {
        let reader = InMemoryLedgerReader::default();
        let results = reader
            .get_entries_by_type_paged("memory.unknown", 0, 100)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn tracks_call_count() {
        let reader = InMemoryLedgerReader::default();
        assert_eq!(reader.call_count(), 0);
        let _ = reader.get_entries_by_type_paged("x", 0, 10).await;
        assert_eq!(reader.call_count(), 1);
        reader.reset_call_count();
        assert_eq!(reader.call_count(), 0);
    }

    #[tokio::test]
    async fn gets_entry_by_id() {
        let reader = InMemoryLedgerReader::default();
        let entry = test_entry("memory.observation.recorded", 1);
        let id = entry.entry_id;
        reader.insert_entry(entry).await;

        let found = reader.get_entry(id).await.unwrap();
        assert_eq!(found.entry_id, id);
    }

    #[tokio::test]
    async fn get_entry_not_found() {
        let reader = InMemoryLedgerReader::default();
        let result = reader.get_entry(Uuid::new_v4()).await;
        assert!(result.is_err());
    }
}
