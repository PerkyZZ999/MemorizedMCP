use anyhow::Result;
use regex::Regex;

pub fn extract_entities(text: &str) -> Vec<String> {
	// Very simple heuristic: capture Capitalized words (length>=3)
	let re = Regex::new(r"\b[A-Z][a-zA-Z]{2,}\b").unwrap();
	let mut entities = Vec::new();
	for cap in re.captures_iter(text) {
		let e = cap.get(0).unwrap().as_str().to_string();
		entities.push(e);
	}
	entities.sort();
	entities.dedup();
	entities
}

pub fn link_entities(db: &sled::Db, doc_id: &str, entities: &[String]) -> Result<()> {
	let ents = db.open_tree("kg_entities")?;
	let links = db.open_tree("kg_links")?;
	for e in entities {
		// Increment entity count
		let cnt = ents.get(e.as_bytes())?.map(|v| u64::from_le_bytes(v.as_ref().try_into().unwrap_or([0u8;8]))).unwrap_or(0);
		let newv = (cnt+1).to_le_bytes();
		ents.insert(e.as_bytes(), &newv)?;
		// Create link doc_id -> entity
		let key = format!("{}::{}", doc_id, e);
		let _ = links.insert(key.as_bytes(), &[]);
	}
	Ok(())
}

// Typed nodes & edges with temporal fields
pub fn ensure_entity_node(db: &sled::Db, name: &str, created_at: i64) -> Result<()> {
	let nodes = db.open_tree("kg_nodes")?;
	let key = format!("Entity::{}", name);
	if nodes.get(key.as_bytes())?.is_none() {
		let val = serde_json::json!({ "type": "Entity", "label": name, "created_at": created_at });
		nodes.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
	}
	Ok(())
}

pub fn ensure_document_node(db: &sled::Db, doc_id: &str, created_at: i64) -> Result<()> {
	let nodes = db.open_tree("kg_nodes")?;
	let key = format!("Document::{}", doc_id);
	if nodes.get(key.as_bytes())?.is_none() {
		let val = serde_json::json!({ "type": "Document", "id": doc_id, "created_at": created_at });
		nodes.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
	}
	Ok(())
}

pub fn ensure_episode_node(db: &sled::Db, episode_id: &str, created_at: i64, name: Option<&str>, session_id: Option<&str>) -> Result<()> {
    let nodes = db.open_tree("kg_nodes")?;
    let key = format!("Episode::{}", episode_id);
    if nodes.get(key.as_bytes())?.is_none() {
        let mut val = serde_json::json!({ "type": "Episode", "id": episode_id, "created_at": created_at });
        if let Some(n) = name { val["name"] = serde_json::json!(n); }
        if let Some(s) = session_id { val["session_id"] = serde_json::json!(s); }
        nodes.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
    }
    Ok(())
}

pub fn add_edge(db: &sled::Db, entity: &str, doc_id: &str, relation: &str, created_at: i64) -> Result<()> {
	let edges = db.open_tree("kg_edges")?;
	let key = format!("{}->{}::{}", entity, doc_id, relation);
	let val = serde_json::json!({ "src": entity, "dst": doc_id, "relation": relation, "created_at": created_at });
	edges.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
	Ok(())
}

pub fn ensure_memory_node(db: &sled::Db, mem_id: &str, created_at: i64) -> Result<()> {
	let nodes = db.open_tree("kg_nodes")?;
	let key = format!("Memory::{}", mem_id);
	if nodes.get(key.as_bytes())?.is_none() {
		let val = serde_json::json!({ "type": "Memory", "id": mem_id, "created_at": created_at });
		nodes.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
	}
	Ok(())
}

pub fn add_edge_generic(db: &sled::Db, src: &str, dst: &str, relation: &str, created_at: i64) -> Result<()> {
	let edges = db.open_tree("kg_edges")?;
	let key = format!("{}->{}::{}", src, dst, relation);
	let val = serde_json::json!({ "src": src, "dst": dst, "relation": relation, "created_at": created_at });
	edges.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
	Ok(())
}

/// Link two documents as RELATED based on shared entities and Jaccard score.
pub fn relate_documents_by_entities(db: &sled::Db, doc_a: &str, doc_b: &str, created_at: i64) -> Result<Option<f32>> {
    let a_ents = super::kg::entities_for_doc(db, doc_a).unwrap_or_default();
    let b_ents = super::kg::entities_for_doc(db, doc_b).unwrap_or_default();
    if a_ents.is_empty() || b_ents.is_empty() { return Ok(None); }
    let a: std::collections::HashSet<String> = a_ents.into_iter().collect();
    let b: std::collections::HashSet<String> = b_ents.into_iter().collect();
    let inter = a.intersection(&b).count() as f32;
    let uni = a.union(&b).count() as f32;
    if uni == 0.0 { return Ok(None); }
    let jacc = inter / uni;
    if jacc > 0.0 {
        let src = format!("Document::{}", doc_a);
        let dst = format!("Document::{}", doc_b);
        let edges = db.open_tree("kg_edges")?;
        let key = format!("{}->{}::RELATED", src, dst);
        let val = serde_json::json!({ "src": src, "dst": dst, "relation": "RELATED", "score": jacc, "created_at": created_at });
        edges.insert(key.as_bytes(), serde_json::to_vec(&val)?)?;
        return Ok(Some(jacc));
    }
    Ok(None)
}

pub fn list_entities(db: &sled::Db, limit: usize) -> Result<Vec<(String, u64)>> {
	let ents = db.open_tree("kg_entities")?;
	let mut pairs: Vec<(String, u64)> = Vec::new();
	for kv in ents.iter() {
		let (k, v) = kv?;
		let name = String::from_utf8(k.to_vec()).unwrap_or_default();
		let cnt = u64::from_le_bytes(v.as_ref().try_into().unwrap_or([0u8;8]));
		pairs.push((name, cnt));
	}
	pairs.sort_by(|a,b| b.1.cmp(&a.1));
	pairs.truncate(limit);
	Ok(pairs)
}

pub fn docs_for_entity(db: &sled::Db, entity: &str) -> Result<Vec<String>> {
	let links = db.open_tree("kg_links")?;
	let mut docs = Vec::new();
	for kv in links.iter() {
		let (k, _) = kv?;
		let key = String::from_utf8(k.to_vec()).unwrap_or_default();
		if key.ends_with(&format!("::{}", entity)) {
			if let Some((doc_id, _)) = key.split_once("::") { docs.push(doc_id.to_string()); }
		}
	}
	docs.sort();
	docs.dedup();
	Ok(docs)
}

pub fn entities_for_doc(db: &sled::Db, doc_id: &str) -> Result<Vec<String>> {
	let links = db.open_tree("kg_links")?;
	let prefix = format!("{}::", doc_id);
	let mut list = Vec::new();
	for kv in links.scan_prefix(prefix.as_bytes()) {
		let (k, _) = kv?;
		let key = String::from_utf8(k.to_vec()).unwrap_or_default();
		if let Some((_, ent)) = key.split_once("::") { list.push(ent.to_string()); }
	}
	Ok(list)
}
