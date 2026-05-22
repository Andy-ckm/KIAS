
pub mod kias_common {
    pub struct KiasError {
        message: String,
    }
    impl KiasError {
        pub fn new(msg: &str) -> Self {
            KiasError { message: msg.to_string() }
        }
    }
    impl std::fmt::Display for KiasError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "KiasError: {}", self.message)
        }
    }
    impl std::error::Error for KiasError {}
}