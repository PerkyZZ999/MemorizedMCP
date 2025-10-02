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

/// Get detailed information about an entity including docs, related entities, and metadata
pub fn get_entity_details(db: &sled::Db, entity: &str) -> Result<serde_json::Value> {
	let nodes = db.open_tree("kg_nodes")?;
	let key = format!("Entity::{}", entity);
	let node_data = nodes.get(key.as_bytes())?
		.and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok())
		.unwrap_or_else(|| serde_json::json!({"type": "Entity", "label": entity}));
	
	// Get documents mentioning this entity
	let docs = docs_for_entity(db, entity).unwrap_or_default();
	
	// Get edges from this entity
	let edges = db.open_tree("kg_edges")?;
	let mut relations: Vec<serde_json::Value> = Vec::new();
	let src_prefix = format!("Entity::{}->", entity);
	for kv in edges.scan_prefix(src_prefix.as_bytes()) {
		if let Ok((_, v)) = kv {
			if let Ok(edge) = serde_json::from_slice::<serde_json::Value>(&v) {
				relations.push(edge);
			}
		}
	}
	
	Ok(serde_json::json!({
		"entity": entity,
		"node": node_data,
		"docs": docs,
		"relations": relations,
		"docCount": docs.len()
	}))
}

/// Search nodes by type and optional label pattern
pub fn search_nodes(db: &sled::Db, node_type: Option<&str>, pattern: Option<&str>, limit: usize) -> Result<Vec<serde_json::Value>> {
	let nodes = db.open_tree("kg_nodes")?;
	let mut results: Vec<serde_json::Value> = Vec::new();
	
	for kv in nodes.iter() {
		if results.len() >= limit { break; }
		let (k, v) = kv?;
		let key = String::from_utf8(k.to_vec()).unwrap_or_default();
		if let Ok(node) = serde_json::from_slice::<serde_json::Value>(&v) {
			// Filter by type if specified
			if let Some(nt) = node_type {
				if node.get("type").and_then(|t| t.as_str()) != Some(nt) {
					continue;
				}
			}
			// Filter by pattern if specified
			if let Some(pat) = pattern {
				let matches = key.to_lowercase().contains(&pat.to_lowercase()) ||
					node.get("label").and_then(|l| l.as_str()).map(|s| s.to_lowercase().contains(&pat.to_lowercase())).unwrap_or(false) ||
					node.get("id").and_then(|l| l.as_str()).map(|s| s.to_lowercase().contains(&pat.to_lowercase())).unwrap_or(false);
				if !matches { continue; }
			}
			let mut result = node.clone();
			result["nodeKey"] = serde_json::json!(key);
			results.push(result);
		}
	}
	
	Ok(results)
}

/// Add tags to an entity node
pub fn tag_entity(db: &sled::Db, entity: &str, tags: &[String]) -> Result<()> {
	let nodes = db.open_tree("kg_nodes")?;
	let key = format!("Entity::{}", entity);
	
	let mut node = nodes.get(key.as_bytes())?
		.and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok())
		.unwrap_or_else(|| {
			let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
			serde_json::json!({"type": "Entity", "label": entity, "created_at": now})
		});
	
	let existing_tags = node.get("tags").and_then(|t| t.as_array()).cloned().unwrap_or_default();
	let mut tag_set: std::collections::HashSet<String> = existing_tags.into_iter()
		.filter_map(|v| v.as_str().map(|s| s.to_string()))
		.collect();
	
	for tag in tags {
		tag_set.insert(tag.clone());
	}
	
	let tags_vec: Vec<serde_json::Value> = tag_set.into_iter().map(|t| serde_json::json!(t)).collect();
	node["tags"] = serde_json::json!(tags_vec);
	
	nodes.insert(key.as_bytes(), serde_json::to_vec(&node)?)?;
	Ok(())
}

/// Remove tags from an entity node
pub fn remove_tags_from_entity(db: &sled::Db, entity: &str, tags: &[String]) -> Result<()> {
	let nodes = db.open_tree("kg_nodes")?;
	let key = format!("Entity::{}", entity);
	
	if let Some(v) = nodes.get(key.as_bytes())? {
		let mut node: serde_json::Value = serde_json::from_slice(&v)?;
		
		if let Some(existing_tags) = node.get("tags").and_then(|t| t.as_array()) {
			let tags_to_remove: std::collections::HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();
			let filtered: Vec<serde_json::Value> = existing_tags.iter()
				.filter(|v| {
					if let Some(tag_str) = v.as_str() {
						!tags_to_remove.contains(tag_str)
					} else {
						true
					}
				})
				.cloned()
				.collect();
			
			node["tags"] = serde_json::json!(filtered);
			nodes.insert(key.as_bytes(), serde_json::to_vec(&node)?)?;
		}
	}
	
	Ok(())
}

/// Get all unique tags across all entities
pub fn get_all_tags(db: &sled::Db) -> Result<Vec<String>> {
	let nodes = db.open_tree("kg_nodes")?;
	let mut tag_set: std::collections::HashSet<String> = std::collections::HashSet::new();
	
	for kv in nodes.iter() {
		if let Ok((_, v)) = kv {
			if let Ok(node) = serde_json::from_slice::<serde_json::Value>(&v) {
				if let Some(tags) = node.get("tags").and_then(|t| t.as_array()) {
					for tag in tags {
						if let Some(tag_str) = tag.as_str() {
							tag_set.insert(tag_str.to_string());
						}
					}
				}
			}
		}
	}
	
	let mut tags: Vec<String> = tag_set.into_iter().collect();
	tags.sort();
	Ok(tags)
}

/// Get entities that have a specific tag
pub fn get_entities_by_tag(db: &sled::Db, tag: &str) -> Result<Vec<String>> {
	let nodes = db.open_tree("kg_nodes")?;
	let mut entities: Vec<String> = Vec::new();
	
	for kv in nodes.iter() {
		if let Ok((k, v)) = kv {
			let key = String::from_utf8(k.to_vec()).unwrap_or_default();
			if key.starts_with("Entity::") {
				if let Ok(node) = serde_json::from_slice::<serde_json::Value>(&v) {
					if let Some(tags) = node.get("tags").and_then(|t| t.as_array()) {
						let has_tag = tags.iter().any(|t| t.as_str() == Some(tag));
						if has_tag {
							if let Some(entity_name) = key.strip_prefix("Entity::") {
								entities.push(entity_name.to_string());
							}
						}
					}
				}
			}
		}
	}
	
	entities.sort();
	Ok(entities)
}

/// Delete an entity node and its edges
pub fn delete_entity(db: &sled::Db, entity: &str) -> Result<u64> {
	let nodes = db.open_tree("kg_nodes")?;
	let edges = db.open_tree("kg_edges")?;
	let ents = db.open_tree("kg_entities")?;
	let links = db.open_tree("kg_links")?;
	
	let key = format!("Entity::{}", entity);
	let mut removed = 0u64;
	
	// Remove node
	if nodes.remove(key.as_bytes())?.is_some() {
		removed += 1;
	}
	
	// Remove from entities count tree
	let _ = ents.remove(entity.as_bytes());
	
	// Remove edges involving this entity
	let src_prefix = format!("Entity::{}->", entity);
	let to_remove_src: Vec<_> = edges.scan_prefix(src_prefix.as_bytes())
		.filter_map(|kv| kv.ok().map(|(k, _)| k))
		.collect();
	for k in to_remove_src {
		let _ = edges.remove(k);
		removed += 1;
	}
	
	// Find and remove edges pointing TO this entity
	for kv in edges.iter() {
		if let Ok((k, v)) = kv {
			if let Ok(edge) = serde_json::from_slice::<serde_json::Value>(&v) {
				if edge.get("dst").and_then(|d| d.as_str()) == Some(&key) {
					let _ = edges.remove(k);
					removed += 1;
				}
			}
		}
	}
	
	// Remove links
	for kv in links.iter() {
		if let Ok((k, _)) = kv {
			let key_str = String::from_utf8(k.to_vec()).unwrap_or_default();
			if key_str.ends_with(&format!("::{}", entity)) {
				let _ = links.remove(k);
			}
		}
	}
	
	Ok(removed)
}

/// Delete a relation/edge
pub fn delete_relation(db: &sled::Db, src: &str, dst: &str, relation: &str) -> Result<bool> {
	let edges = db.open_tree("kg_edges")?;
	let key = format!("{}->{}::{}", src, dst, relation);
	Ok(edges.remove(key.as_bytes())?.is_some())
}
