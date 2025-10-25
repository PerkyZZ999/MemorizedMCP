pub const EMBED_DIM: usize = 384;

#[cfg(not(feature = "fastembed"))]
pub fn embed_batch(texts: &[&str]) -> Vec<[f32; EMBED_DIM]> {
    texts.iter().map(|_| [0.0; EMBED_DIM]).collect()
}

#[cfg(feature = "fastembed")]
pub fn embed_batch(texts: &[&str]) -> Vec<[f32; EMBED_DIM]> {
    // TODO: integrate fastembed actual embeddings here
    texts.iter().map(|_| [0.0; EMBED_DIM]).collect()
}
