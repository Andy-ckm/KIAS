
pub struct CachedPrompt {
    pub prompt: String,
    pub response: String,
    pub embedding: Option<Vec<f32>>,
}