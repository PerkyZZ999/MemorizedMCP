use crate::embeddings::EMBED_DIM;
use anyhow::Result;
use std::cmp::Ordering;

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len().min(b.len()) {
        let x = a[i];
        let y = b[i];
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

pub fn record_vectors(
    db: &sled::Db,
    doc_id: &str,
    chunk_starts: &[usize],
    vector_dim: usize,
) -> Result<()> {
    let meta = db.open_tree("vec_meta")?;
    let items_key = b"items";
    let dim_key = b"dim";
    // update items count
    let prev = meta
        .get(items_key)?
        .map(|v| u64::from_le_bytes(v.as_ref().try_into().unwrap_or([0u8; 8])))
        .unwrap_or(0);
    let newv = (prev + chunk_starts.len() as u64).to_le_bytes();
    meta.insert(items_key, &newv)?;
    // set dim
    let dim_bytes = (vector_dim as u64).to_le_bytes();
    meta.insert(dim_key, &dim_bytes)?;
    // record simple postings: doc_id -> number of vectors (for scaffold)
    let key = format!("doc::{}", doc_id);
    let val = (chunk_starts.len() as u64).to_le_bytes();
    meta.insert(key.as_bytes(), &val)?;
    Ok(())
}

/// Search memory embeddings by cosine similarity. Returns (id, score) top_k.
pub fn search_memories_by_vector(db: &sled::Db, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
    let mut hits: Vec<(String, f32)> = Vec::new();
    if let Ok(tree) = db.open_tree("mem_embeddings") {
        for kv in tree.iter() {
            if let Ok((k, v)) = kv {
                let id = String::from_utf8_lossy(&k).to_string();
                // Validate dimension
                if v.len() != EMBED_DIM * 4 {
                    continue;
                }
                let emb: &[f32] = bytemuck::cast_slice(&v);
                let score = cosine_similarity(query, emb);
                hits.push((id, score));
            }
        }
    }
    hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(top_k);
    hits
}

/// Remove mem_embeddings entries whose memory record no longer exists.
pub fn cleanup_orphan_mem_embeddings(db: &sled::Db) -> Result<u64> {
    let emb = db.open_tree("mem_embeddings")?;
    let mems = db.open_tree("memories")?;
    let mut removed: u64 = 0;
    for kv in emb.iter() {
        let (k, _) = kv?;
        if mems.get(&k)?.is_none() {
            let _ = emb.remove(&k)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Validate embedding dimensions; returns (total, invalid) counts.
pub fn validate_mem_embeddings(db: &sled::Db) -> (u64, u64) {
    let mut total: u64 = 0;
    let mut invalid: u64 = 0;
    if let Ok(tree) = db.open_tree("mem_embeddings") {
        for kv in tree.iter() {
            if let Ok((_, v)) = kv {
                total += 1;
                if v.len() != EMBED_DIM * 4 {
                    invalid += 1;
                }
            }
        }
    }
    (total, invalid)
}

fn get_mem_embedding(db: &sled::Db, id: &str) -> Option<Vec<f32>> {
    if let Ok(tree) = db.open_tree("mem_embeddings") {
        if let Ok(Some(v)) = tree.get(id.as_bytes()) {
            if v.len() != EMBED_DIM * 4 {
                return None;
            }
            let slice: &[f32] = bytemuck::cast_slice(&v);
            return Some(slice.to_vec());
        }
    }
    None
}

/// Build a neighbor graph for memories (HNSW-like single layer), storing top-M neighbors by cosine.
pub fn build_mem_neighbor_graph(db: &sled::Db, m_neighbors: usize) -> Result<u64> {
    let emb = db.open_tree("mem_embeddings")?;
    let mut ids: Vec<String> = Vec::new();
    let mut vecs: Vec<Vec<f32>> = Vec::new();
    for kv in emb.iter() {
        let (k, v) = kv?;
        if v.len() != EMBED_DIM * 4 {
            continue;
        }
        ids.push(String::from_utf8_lossy(&k).to_string());
        let sl: &[f32] = bytemuck::cast_slice(&v);
        vecs.push(sl.to_vec());
    }
    let n = ids.len();
    if n == 0 {
        return Ok(0);
    }
    let neigh = db.open_tree("hnsw_mem_neighbors")?;
    use rayon::prelude::*;
    let entries: Vec<(Vec<u8>, Vec<u8>)> = (0..n)
        .into_par_iter()
        .map(|i| {
            let a = &vecs[i];
            let mut top: Vec<(f32, usize)> = Vec::with_capacity(m_neighbors + 1);
            for j in 0..n {
                if i == j {
                    continue;
                }
                let score = cosine_similarity(a, &vecs[j]);
                if top.len() < m_neighbors {
                    top.push((score, j));
                } else {
                    top.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(Ordering::Equal));
                    if let Some((min_score, _)) = top.first() {
                        if score > *min_score {
                            top[0] = (score, j);
                        }
                    }
                }
            }
            top.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
            let arr: Vec<serde_json::Value> = top
                .into_iter()
                .map(|(s, idx)| serde_json::json!({ "id": ids[idx], "score": s }))
                .collect();
            (
                ids[i].as_bytes().to_vec(),
                serde_json::to_vec(&arr).unwrap_or_default(),
            )
        })
        .collect();
    for (k, v) in entries {
        neigh.insert(k, v)?;
    }
    let edges_written: u64 = n as u64;
    Ok(edges_written)
}

/// ANN search over the neighbor graph; falls back to brute force if graph missing.
pub fn ann_search_memories(db: &sled::Db, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
    let neigh = db.open_tree("hnsw_mem_neighbors");
    if neigh.is_err() {
        return search_memories_by_vector(db, query, top_k);
    }
    let neigh = neigh.unwrap();
    // choose entry: pick first with highest sim among first 16 entries
    let emb = db.open_tree("mem_embeddings");
    if emb.is_err() {
        return Vec::new();
    }
    let emb = emb.unwrap();
    let mut entry_id: Option<String> = None;
    let mut best_sim = -1.0f32;
    for (idx, kv) in emb.iter().enumerate() {
        if idx >= 16 {
            break;
        }
        if let Ok((k, v)) = kv {
            if v.len() == EMBED_DIM * 4 {
                let id = String::from_utf8_lossy(&k).to_string();
                let vec: &[f32] = bytemuck::cast_slice(&v);
                let s = cosine_similarity(query, vec);
                if s > best_sim {
                    best_sim = s;
                    entry_id = Some(id);
                }
            }
        }
    }
    let entry = match entry_id {
        Some(e) => e,
        None => return Vec::new(),
    };
    // greedy search
    use std::collections::{BinaryHeap, HashSet};
    #[derive(PartialEq)]
    struct Scored {
        score: f32,
        id: String,
    }
    impl Eq for Scored {}
    impl Ord for Scored {
        fn cmp(&self, other: &Self) -> Ordering {
            self.score
                .partial_cmp(&other.score)
                .unwrap_or(Ordering::Equal)
        }
    }
    impl PartialOrd for Scored {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    let mut visited: HashSet<String> = HashSet::new();
    let mut best: BinaryHeap<Scored> = BinaryHeap::new();
    let mut frontier: Vec<String> = vec![entry.clone()];
    while let Some(cur) = frontier.pop() {
        if !visited.insert(cur.clone()) {
            continue;
        }
        if let Some(vec) = get_mem_embedding(db, &cur) {
            let s = cosine_similarity(query, &vec);
            best.push(Scored {
                score: s,
                id: cur.clone(),
            });
            if best.len() > top_k {
                best.pop();
            }
        }
        if let Ok(Some(nv)) = neigh.get(cur.as_bytes()) {
            if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&nv) {
                for item in arr.into_iter().take(8) {
                    // limit branching
                    if let Some(nid) = item.get("id").and_then(|x| x.as_str()) {
                        frontier.push(nid.to_string());
                    }
                }
            }
        }
        if visited.len() > 1024 {
            break;
        }
    }
    let mut out: Vec<(String, f32)> = best
        .into_sorted_vec()
        .into_iter()
        .rev()
        .map(|s| (s.id, s.score))
        .collect();
    out.truncate(top_k);
    out
}

/// Re-embed all memories in batches using embed_batch.
pub fn reembed_all_memories(db: &sled::Db, batch_size: usize) -> Result<u64> {
    let mems = db.open_tree("memories")?;
    let mut ids: Vec<String> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    for kv in mems.iter() {
        let (_k, v) = kv?;
        if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
            if let Some(id) = rec.get("id").and_then(|x| x.as_str()) {
                ids.push(id.to_string());
                texts.push(
                    rec.get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                );
            }
        }
    }
    let emb = db.open_tree("mem_embeddings")?;
    let mut written: u64 = 0;
    let mut i = 0usize;
    while i < ids.len() {
        let end = (i + batch_size).min(ids.len());
        let slice = &texts[i..end];
        let refs: Vec<&str> = slice.iter().map(|s| s.as_str()).collect();
        let vecs = crate::embeddings::embed_batch(&refs);
        for (j, id) in ids[i..end].iter().enumerate() {
            let bytes: &[u8] = bytemuck::cast_slice(&vecs[j]);
            emb.insert(id.as_bytes(), bytes)?;
            written += 1;
        }
        i = end;
    }
    Ok(written)
}
