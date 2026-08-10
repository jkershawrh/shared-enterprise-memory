pub mod memory;
pub mod postgres;
pub mod types;

pub use memory::InMemoryLedgerReader;
pub use types::LedgerEntry;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LedgerReadError {
    #[error("ledger unavailable")]
    Unavailable,
    #[error("entry not found: {0}")]
    NotFound(Uuid),
}

#[async_trait]
pub trait LedgerReader: Send + Sync {
    async fn get_entries_by_type_paged(
        &self,
        entry_type: &str,
        after_position: i64,
        limit: i64,
    ) -> Result<Vec<LedgerEntry>, LedgerReadError>;

    async fn get_entry(&self, entry_id: Uuid) -> Result<LedgerEntry, LedgerReadError>;
}
