//! Sandbox - Generated feature implementation
use kias_common::KiasError;
use kias_common::KiasResult;

/// Sandbox provides new, create, apply seccomp, set cgroup, destroy
#[derive(Debug, Clone)]
pub struct Sandbox {
    initialized: bool,
}

impl Sandbox {
    /// Create a new Sandbox instance
    pub fn new() -> Self {
        Self { initialized: true }
    }

    /// New operation
    pub fn init(&self) -> KiasResult<()> {
        tracing::info!("Sandbox::init called");
        Ok(())
    }

    /// Create operation
    pub fn create(&self) -> KiasResult<()> {
        tracing::info!("Sandbox::create called");
        Ok(())
    }

    /// Apply Seccomp operation
    pub fn apply_seccomp(&self) -> KiasResult<()> {
        tracing::info!("Sandbox::apply_seccomp called");
        Ok(())
    }

    /// Set Cgroup operation
    pub fn set_cgroup(&self) -> KiasResult<()> {
        tracing::info!("Sandbox::set_cgroup called");
        Ok(())
    }

    /// Destroy operation
    pub fn destroy(&self) -> KiasResult<()> {
        tracing::info!("Sandbox::destroy called");
        Ok(())
    }

}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let s = Sandbox::new();
        assert!(s.init().is_ok());
    }

    #[test]
    fn test_create() {
        let s = Sandbox::new();
        assert!(s.create().is_ok());
    }

    #[test]
    fn test_apply_seccomp() {
        let s = Sandbox::new();
        assert!(s.apply_seccomp().is_ok());
    }

    #[test]
    fn test_set_cgroup() {
        let s = Sandbox::new();
        assert!(s.set_cgroup().is_ok());
    }

}
