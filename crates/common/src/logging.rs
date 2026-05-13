//! Tracing / logging initialisation for KIAS.
//!
//! Call [`init_logging`] once near the top of every binary's `main()`.

use crate::config::LoggingConfig;

/// Initialise the global tracing subscriber based on [`LoggingConfig`].
///
/// * `level` – one of `trace`, `debug`, `info`, `warn`, `error`.
/// * `format` – `text` for human-readable output, `json` for structured logs.
///
/// This function is **idempotent**: calling it more than once is a no-op
/// (the subscriber is set once globally).
pub fn init_logging(config: &LoggingConfig) {
    init_logging_with_level(&config.level, &config.format);
}

/// Lower-level helper that accepts raw strings.
pub fn init_logging_with_level(level: &str, format: &str) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    match format {
        "json" => {
            let _ = fmt()
                .with_env_filter(filter)
                .json()
                .try_init();
        }
        _ => {
            let _ = fmt()
                .with_env_filter(filter)
                .with_target(true)
                .with_thread_ids(true)
                .compact()
                .try_init();
        }
    }

    tracing::info!(level, format, "Logging initialised");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_does_not_panic() {
        // init_logging may fail silently on repeated calls; it must never panic.
        init_logging_with_level("info", "text");
        init_logging_with_level("debug", "json");
    }
}
