//! # Text Embedder
//!
//! Converts text (capability names, skill descriptions, intent strings)
//! into fixed-dimension embedding vectors for semantic similarity search.
//!
//! ## Approach: Feature Hashing (Hashing Trick)
//!
//! Inspired by Sembr, we use character-level n-gram feature hashing to produce
//! deterministic, fixed-dimension embeddings from arbitrary text. This avoids
//! external dependencies (no ML model, no API calls) while capturing sub-word
//! semantic similarity.
//!
//! ### Algorithm
//!
//! 1. Tokenize text into word-level n-grams (unigrams + bigrams)
//! 2. Hash each n-gram to a bucket in `[0, DIM)`
//! 3. Accumulate signed values (+1/-1 based on hash parity)
//! 4. L2-normalize the result
//!
//! This gives us:
//! - **Same domain terms** → similar vectors (e.g., "code_generation" ≈ "code_review")
//! - **Synonyms** partially captured via shared n-grams
//! - **O(n) time** where n = text length
//!
//! ## Dimension Choice
//!
//! Default 128 dimensions balances quality vs. memory. For <1000 skills,
//! 64 dimensions would also work. For >10K skills, consider 256+.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Default embedding dimension for skill matching.
/// Balances recall quality vs. memory usage for typical agent fleets (<1000 agents).
pub const DEFAULT_EMBEDDING_DIM: usize = 128;

/// Trait for text-to-vector embedding.
///
/// Implementors convert arbitrary text into a fixed-dimension f32 vector
/// suitable for cosine similarity comparison.
pub trait Embedder: Send + Sync {
    /// Embed text into a fixed-dimension vector.
    fn embed(&self, text: &str) -> Vec<f32>;

    /// Embedding dimension.
    fn dimension(&self) -> usize;
}

/// Feature-hashing embedder using character-level n-grams.
///
/// Produces deterministic embeddings without external dependencies.
/// Captures sub-word similarity via overlapping character n-grams.
pub struct HashingEmbedder {
    dimension: usize,
    ngram_sizes: Vec<usize>,
}

impl HashingEmbedder {
    /// Create a new embedder with default settings (dim=128, n-grams 2..=4).
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            ngram_sizes: vec![2, 3, 4],
        }
    }

    /// Create an embedder with custom n-gram sizes.
    pub fn with_ngram_sizes(mut self, sizes: Vec<usize>) -> Self {
        self.ngram_sizes = sizes;
        self
    }

    /// Extract character n-grams from text.
    fn extract_ngrams(&self, text: &str) -> Vec<String> {
        let normalized: String = text
            .to_lowercase()
            .chars()
            .map(|c| {
                if c == '_' || c == '-' || c == '.' {
                    ' '
                } else {
                    c
                }
            })
            .collect();

        // Also add word-level tokens (unigrams)
        let words: Vec<&str> = normalized.split_whitespace().collect();
        let mut ngrams: Vec<String> = words.iter().map(|w| w.to_string()).collect();

        // Character n-grams from the full normalized text
        let chars: Vec<char> = normalized.chars().collect();
        for &n in &self.ngram_sizes {
            if chars.len() < n {
                continue;
            }
            for window in chars.windows(n) {
                let gram: String = window.iter().collect();
                // Skip ngrams that are pure whitespace
                if gram.chars().all(|c| c.is_whitespace()) {
                    continue;
                }
                ngrams.push(gram);
            }
        }

        ngrams
    }

    /// Hash a token to a bucket index and sign.
    /// Returns (bucket_index, sign: +1.0 or -1.0).
    fn hash_to_bucket(&self, token: &str) -> (usize, f32) {
        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let hash = hasher.finish();

        let bucket = (hash as usize) % self.dimension;
        // Use the next bit for sign (murmurhash-style)
        let sign = if (hash >> 32) & 1 == 0 { 1.0 } else { -1.0 };

        (bucket, sign)
    }
}

impl Embedder for HashingEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimension];
        let ngrams = self.extract_ngrams(text);

        for ngram in &ngrams {
            let (bucket, sign) = self.hash_to_bucket(ngram);
            vector[bucket] += sign;
        }

        // L2-normalize
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }

        vector
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Convenience: embed two texts and return their cosine similarity.
pub fn text_similarity(embedder: &dyn Embedder, a: &str, b: &str) -> f32 {
    let va = embedder.embed(a);
    let vb = embedder.embed(b);
    cosine_similarity(&va, &vb)
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }

    (dot / (na * nb)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dimension() {
        let embedder = HashingEmbedder::new(128);
        let vec = embedder.embed("code_generation");
        assert_eq!(vec.len(), 128);
    }

    #[test]
    fn test_embedding_normalized() {
        let embedder = HashingEmbedder::new(64);
        let vec = embedder.embed("testing");
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Expected unit norm, got {norm}");
    }

    #[test]
    fn test_similar_texts_high_similarity() {
        let embedder = HashingEmbedder::new(128);
        let sim = text_similarity(&embedder, "code_generation", "code_review");
        // Feature hashing on short capability names: 0.2+ is meaningful overlap
        // (random text would be ~0.0)
        assert!(
            sim > 0.15,
            "Similar skills should have decent similarity, got {sim}"
        );
    }

    #[test]
    fn test_same_text_perfect_similarity() {
        let embedder = HashingEmbedder::new(128);
        let sim = text_similarity(&embedder, "web_search", "web_search");
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Same text should be ~1.0, got {sim}"
        );
    }

    #[test]
    fn test_different_domain_lower_similarity() {
        let embedder = HashingEmbedder::new(128);
        let sim_same = text_similarity(&embedder, "code_generation", "code_review");
        let sim_diff = text_similarity(&embedder, "code_generation", "sql_query");
        // Same domain should be more similar than different domain
        assert!(
            sim_same > sim_diff,
            "Same domain ({sim_same}) should beat different domain ({sim_diff})"
        );
    }

    #[test]
    fn test_underscore_normalization() {
        let embedder = HashingEmbedder::new(128);
        // "code_generation" and "code generation" should be identical after normalization
        let v1 = embedder.embed("code_generation");
        let v2 = embedder.embed("code generation");
        let sim = cosine_similarity(&v1, &v2);
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Underscore and space should normalize identically, got {sim}"
        );
    }

    #[test]
    fn test_deterministic() {
        let embedder = HashingEmbedder::new(64);
        let v1 = embedder.embed("test");
        let v2 = embedder.embed("test");
        assert_eq!(v1, v2, "Embeddings should be deterministic");
    }

    #[test]
    fn test_custom_ngram_sizes() {
        let embedder = HashingEmbedder::new(64).with_ngram_sizes(vec![2, 3]);
        let vec = embedder.embed("hello world");
        assert_eq!(vec.len(), 64);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(norm > 0.0, "Should produce non-zero embedding");
    }

    #[test]
    fn test_empty_text() {
        let embedder = HashingEmbedder::new(32);
        let vec = embedder.embed("");
        assert_eq!(vec.len(), 32);
        // All zeros for empty text
        assert!(vec.iter().all(|&x| x == 0.0));
    }
}
