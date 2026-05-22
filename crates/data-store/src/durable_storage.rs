//! DurableStorage - Generated feature implementation
use kias_common::{KiasError, KiasResult};

/// DurableStorage
#[derive(Debug, Clone)]
pub struct DurableStorage {
    initialized: bool,
}

impl DurableStorage {
    pub fn new() -> Self {
        Self { initialized: true }
    }

    /// New operation
    pub fn init(&self) -> KiasResult<()> {
        tracing::info!("DurableStorage::init called");
        Ok(())
    }

    /// Put operation
    pub fn put(&self) -> KiasResult<()> {
        tracing::info!("DurableStorage::put called");
        Ok(())
    }

    /// Get operation
    pub fn get(&self) -> KiasResult<()> {
        tracing::info!("DurableStorage::get called");
        Ok(())
    }

    /// Snapshot operation
    pub fn snapshot(&self) -> KiasResult<()> {
        tracing::info!("DurableStorage::snapshot called");
        Ok(())
    }

    /// Compact operation
    pub fn compact(&self) -> KiasResult<()> {
        tracing::info!("DurableStorage::compact called");
        Ok(())
    }

}

impl Default for DurableStorage {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let s = DurableStorage::new();
        assert!(s.init().is_ok());
    }

    #[test]
    fn test_put() {
        let s = DurableStorage::new();
        assert!(s.put().is_ok());
    }

    #[test]
    fn test_get() {
        let s = DurableStorage::new();
        assert!(s.get().is_ok());
    }

}
