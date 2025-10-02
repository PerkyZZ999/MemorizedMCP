use std::{
    sync::{Arc, Mutex as StdMutex},
    time::Instant,
    collections::HashMap,
    collections::HashSet,
    collections::VecDeque,
};

use anyhow::Result;
use axum::{routing::{get, post}, Json, Router, response::{IntoResponse, Response}};
use tower_http::trace::TraceLayer;
use axum::http::StatusCode;
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sled::Db;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use pulldown_cmark::{Event as MdEvent, Options as MdOptions, Parser as MdParser};
use lopdf::Document as LoDocument;
use tokio::{io::{AsyncBufReadExt, BufReader}, signal, task, time::{sleep, Duration}};
use tokio::sync::Semaphore;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use tokio::sync::Mutex as AsyncMutex;

mod config;
mod embeddings;
mod kg;
mod vector_index;

#[derive(Parser, Debug)]
#[command(name = "memory-mcp-server", version, about = "MCP Server for Enhanced Memory")] 
struct Cli {
	/// Bind address for HTTP server (set empty to disable HTTP)
	#[arg(long, env = "HTTP_BIND", default_value = "127.0.0.1:8080")]
	bind: String,

	/// Data directory root
	#[arg(long, env = "DATA_DIR", default_value = "./data")]
	data_dir: String,
}

struct AppState {
	start_time: Instant,
	db: Db,
	index_dir: std::path::PathBuf,
	// Query cache for hot fusion queries: key -> (ts_ms, results)
	query_cache: AsyncMutex<HashMap<String, (i64, Vec<SearchResult>)>>,
    metrics: AsyncMutex<QueryMetrics>,
    ingest_sema: Arc<Semaphore>,
	// Simple buffer pool to reuse byte buffers on hot paths
    #[allow(dead_code)]
    buf_pool: StdMutex<ByteBufPool>,
}

#[derive(Default)]
struct ByteBufPool { #[allow(dead_code)] default_capacity: usize }

impl ByteBufPool {
    #[allow(dead_code)]
    fn new(default_capacity: usize) -> Self { Self { default_capacity } }
}

#[derive(Deserialize)]
struct StoreDocRequest {
	path: Option<String>,
	mime: Option<String>,
	content: Option<String>,
	metadata: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct StoreDocResponse { id: String, hash: String, chunks: usize }

#[derive(Serialize, Deserialize)]
struct ChunkHeader { id: String, position: Position }

#[derive(Serialize, Deserialize)]
struct Position { start: usize, end: usize }

#[derive(Serialize)]
struct Health { status: &'static str }

#[derive(Serialize, Deserialize)]
struct RefInput {
	#[serde(rename = "docId")]
	doc_id: String,
	#[serde(rename = "chunkId")]
	#[serde(default)]
	chunk_id: Option<String>,
	#[serde(default)]
	score: Option<f32>,
}

#[derive(Deserialize)]
struct AddMemoryRequest {
    #[serde(deserialize_with = "deserialize_content_to_string")]
    content: String,
    metadata: Option<JsonValue>,
    layer_hint: Option<String>,
    session_id: Option<String>,
    episode_id: Option<String>,
    #[serde(default)]
    references: Option<Vec<RefInput>>,
}

#[derive(Serialize)]
struct AddMemoryResponse { id: String, layer: String }

#[derive(Serialize, Clone)]
struct DocRefOut {
	#[serde(rename = "docId")]
	doc_id: String,
	#[serde(rename = "chunkId")]
	chunk_id: Option<String>,
	score: Option<f32>,
}

#[derive(Serialize, Clone)]
struct SearchResult {
    id: String,
    score: f32,
    layer: String,
    #[serde(rename = "docRefs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    doc_refs: Option<Vec<DocRefOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    explain: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct SearchResponse { results: Vec<SearchResult>, #[serde(rename = "tookMs")] #[serde(skip_serializing_if = "Option::is_none")] took_ms: Option<u128> }

#[derive(Deserialize)]
struct UpdateMemoryRequest { id: String, content: Option<String>, metadata: Option<JsonValue> }

#[derive(Deserialize)]
struct DeleteMemoryRequest { id: String, #[serde(default)] backup: Option<bool> }

fn deserialize_content_to_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    match value {
        JsonValue::String(s) => Ok(s),
        JsonValue::Null => Ok(String::new()),
        other => serde_json::to_string(&other).map_err(serde::de::Error::custom),
    }
}

#[derive(Serialize, Default)]
struct IndicesStatus { 
	vector: VectorIndexStatus,
	text: TextIndexStatus,
	graph: GraphIndexStatus,
}

#[derive(Serialize, Default)]
struct VectorIndexStatus { items: u64 }

#[derive(Serialize, Default)]
struct TextIndexStatus { docs: u64 }

#[derive(Serialize, Default)]
struct GraphIndexStatus { nodes: u64, edges: u64 }

#[derive(Serialize, Default)]
struct StorageStatus { hot_mb: u64, warm_mb: u64, cold_mb: u64 }

#[derive(Serialize, Default, Clone)]
struct QueryMetrics {
    count: u64,
    #[serde(rename = "cacheHits")]
    cache_hits: u64,
    #[serde(rename = "cacheMisses")]
    cache_misses: u64,
    #[serde(rename = "avgMs")]
    avg_ms: f64,
    #[serde(rename = "lastMs")]
    last_ms: u64,
    #[serde(rename = "p50Ms")]
    p50_ms: f64,
    #[serde(rename = "p95Ms")]
    p95_ms: f64,
    #[serde(rename = "qps1m")]
    qps_1m: f64,
    #[serde(skip)]
    history: VecDeque<(i64, u64)>,
}

#[derive(Serialize)]
struct StatusResponse {
	uptime_ms: u128,
	indices: IndicesStatus,
	storage: StorageStatus,
    metrics: QueryMetrics,
	#[serde(rename = "memory")]
	proc_mem: ProcMem,
	health: &'static str,
}

#[derive(Serialize, Default, Clone)]
struct ProcMem {
    rss_mb: u64,
    stm_count: u64,
    ltm_count: u64,
}

#[derive(Serialize)]
struct ToolDescriptor { name: &'static str, description: &'static str }
#[inline]
fn json_error(status: StatusCode, code: &'static str, message: impl Into<String>, details: Option<serde_json::Value>) -> Response {
    let body = serde_json::json!({ "error": { "code": code, "message": message.into(), "details": details } });
    (status, Json(body)).into_response()
}


#[tokio::main]
async fn main() -> Result<()> {
	init_tracing();
	let env_cfg = config::Config::load().unwrap_or_else(|_| config::Config { bind: "127.0.0.1:8080".parse().unwrap(), data_dir: "./data".to_string() });
	let cli = Cli::parse();

	let data_dir = if cli.data_dir != "./data" { cli.data_dir.clone() } else { env_cfg.data_dir.clone() };
	let bind_addr: std::net::SocketAddr = if cli.bind != "127.0.0.1:8080" { cli.bind.parse().expect("Invalid bind") } else { env_cfg.bind };

	let dirs = ensure_data_dirs(&data_dir)?;
	let db_path = dirs.warm.join("kv");
	let db = sled::open(db_path)?;

	// Initialize persistent settings KV with effective config
	{
		let settings = db.open_tree("settings")?;
		let _ = settings.insert(b"effective_bind", bind_addr.to_string().as_bytes());
		let _ = settings.insert(b"data_dir", data_dir.as_bytes());
	}

	let state = Arc::new(AppState {
		start_time: Instant::now(),
		db,
		index_dir: dirs.index,
		query_cache: AsyncMutex::new(HashMap::new()),
        metrics: AsyncMutex::new(QueryMetrics::default()),
        ingest_sema: Arc::new(Semaphore::new(std::env::var("MAX_CONCURRENT_INGEST").ok().and_then(|v| v.parse().ok()).unwrap_or(4))),
		buf_pool: StdMutex::new(ByteBufPool::default()),
	});

	let mut tasks = Vec::new();

	// Maintenance loop (STM eviction, LTM decay)
	let maint_state = state.clone();
	let maint_task = task::spawn(async move { maintenance_loop(maint_state).await; });
	tasks.push(maint_task);

	// HTTP server (if bind not empty)
	if !cli.bind.is_empty() {
		let http_state = state.clone();
		info!(%bind_addr, "Starting HTTP server");
		let http_task = task::spawn(async move {
			let app = build_router(http_state);
			let listener = tokio::net::TcpListener::bind(bind_addr).await.expect("bind failed");
			axum::serve(listener, app)
				.with_graceful_shutdown(shutdown_signal())
				.await
				.expect("server error");
		});
		tasks.push(http_task);
	}

	// STDIO stub (non-blocking)
	{
		let stdio_state = state.clone();
		let stdio_task = task::spawn(async move { run_stdio(stdio_state).await; });
		tasks.push(stdio_task);
	}

	// Wait for Ctrl+C then exit
	signal::ctrl_c().await?;
	info!("Shutdown signal received");
	for t in tasks { let _ = t.abort(); }
	Ok(())
}

fn init_tracing() {
	let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // Write all tracing logs to stderr so stdout stays JSON-RPC clean for MCP
    let fmt_layer = fmt::layer().with_target(false).with_ansi(false).with_writer(std::io::stderr);
    tracing_subscriber::registry().with(env_filter).with(fmt_layer).init();
}

fn ensure_data_dirs(root: &str) -> Result<DataDirs> {
	use std::fs;
	use std::path::PathBuf;
	let root = PathBuf::from(root);
	let hot = root.join("hot");
	let warm = root.join("warm");
	let cold = root.join("cold");
	let index = root.join("index");
	fs::create_dir_all(&hot)?;
	fs::create_dir_all(&warm)?;
	fs::create_dir_all(&cold)?;
	fs::create_dir_all(&index)?;
	Ok(DataDirs { warm, index })
}

struct DataDirs { warm: std::path::PathBuf, index: std::path::PathBuf }

fn build_router(state: Arc<AppState>) -> Router {
	Router::new()
		.route("/health", get(health))
		.route("/status", get(status))
        .route("/metrics", get(metrics_route))
		.route("/tools", get(list_tools_route))
		.route("/document/store", post(document_store))
		.route("/document/retrieve", get(document_retrieve))
		.route("/document/analyze", get(document_analyze))
		.route("/document/refs_for_memory", get(document_refs_for_memory))
		.route("/document/refs_for_document", get(document_refs_for_document))
		.route("/document/validate_refs", post(document_validate_refs))
		.route("/kg/entities", get(kg_entities))
		.route("/kg/docs_for_entity", get(kg_docs_for_entity))
		.route("/kg/snapshot", get(kg_snapshot))
		.route("/kg/list_entities", get(kg_list_entities))
		.route("/kg/get_entity", get(kg_get_entity))
		.route("/kg/create_entity", post(kg_create_entity))
		.route("/kg/create_relation", post(kg_create_relation))
		.route("/kg/search_nodes", get(kg_search_nodes))
		.route("/kg/read_graph", get(kg_read_graph))
		.route("/kg/tag_entity", post(kg_tag_entity))
		.route("/kg/get_tags", get(kg_get_tags))
		.route("/kg/remove_tag", post(kg_remove_tag))
		.route("/kg/delete_entity", post(kg_delete_entity))
		.route("/kg/delete_relation", post(kg_delete_relation))
		.route("/memory/add", post(memory_add))
		.route("/memory/search", get(memory_search))
		.route("/memory/update", post(memory_update))
		.route("/memory/delete", post(memory_delete))
		.route("/search/fusion", get(search_fusion))
		.route("/advanced/consolidate", post(advanced_consolidate))
        .route("/advanced/reindex", post(advanced_reindex))
        .route("/advanced/analyze_patterns", post(advanced_analyze_patterns))
        .route("/advanced/trends", post(advanced_trends))
        .route("/advanced/clusters", post(advanced_clusters))
        .route("/advanced/relationships", post(advanced_relationships))
        .route("/advanced/effectiveness", post(advanced_effectiveness))
		.route("/system/cleanup", post(system_cleanup))
        .route("/system/backup", post(system_backup))
        .route("/system/restore", post(system_restore))
        .route("/system/compact", post(system_compact))
        .route("/system/validate", get(system_validate))
        .route("/data/export", post(data_export))
        .route("/data/import", post(data_import))
        .layer(TraceLayer::new_for_http())
		.with_state(state)
}

async fn proxy_tool_via_http(tool_name: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let bind = std::env::var("HTTP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let base = format!("http://{}", bind);
    // Map tool names to method and path (support both dot and underscore notation)
    let (method, path) = match tool_name {
        // Memory (dot notation)
        "memory.add" => ("POST", "/memory/add"),
        "memory.search" => ("GET", "/memory/search"),
        "memory.update" => ("POST", "/memory/update"),
        "memory.delete" => ("POST", "/memory/delete"),
        // Memory (underscore notation)
        "memory_add" => ("POST", "/memory/add"),
        "memory_search" => ("GET", "/memory/search"),
        "memory_update" => ("POST", "/memory/update"),
        "memory_delete" => ("POST", "/memory/delete"),
        // Document (dot notation)
        "document.store" => ("POST", "/document/store"),
        "document.retrieve" => ("GET", "/document/retrieve"),
        "document.analyze" => ("GET", "/document/analyze"),
        "document.refs_for_memory" => ("GET", "/document/refs_for_memory"),
        "document.refs_for_document" => ("GET", "/document/refs_for_document"),
        "document.validate_refs" => ("POST", "/document/validate_refs"),
        // Document (underscore notation)
        "document_store" => ("POST", "/document/store"),
        "document_retrieve" => ("GET", "/document/retrieve"),
        "document_analyze" => ("GET", "/document/analyze"),
        "document_refs_for_memory" => ("GET", "/document/refs_for_memory"),
        "document_refs_for_document" => ("GET", "/document/refs_for_document"),
        "document_validate_refs" => ("POST", "/document/validate_refs"),
        // Knowledge Graph (dot notation)
        "kg.list_entities" => ("GET", "/kg/list_entities"),
        "kg.get_entity" => ("GET", "/kg/get_entity"),
        "kg.create_entity" => ("POST", "/kg/create_entity"),
        "kg.create_relation" => ("POST", "/kg/create_relation"),
        "kg.search_nodes" => ("GET", "/kg/search_nodes"),
        "kg.read_graph" => ("GET", "/kg/read_graph"),
        "kg.tag_entity" => ("POST", "/kg/tag_entity"),
        "kg.get_tags" => ("GET", "/kg/get_tags"),
        "kg.remove_tag" => ("POST", "/kg/remove_tag"),
        "kg.delete_entity" => ("POST", "/kg/delete_entity"),
        "kg.delete_relation" => ("POST", "/kg/delete_relation"),
        // Knowledge Graph (underscore notation)
        "kg_list_entities" => ("GET", "/kg/list_entities"),
        "kg_get_entity" => ("GET", "/kg/get_entity"),
        "kg_create_entity" => ("POST", "/kg/create_entity"),
        "kg_create_relation" => ("POST", "/kg/create_relation"),
        "kg_search_nodes" => ("GET", "/kg/search_nodes"),
        "kg_read_graph" => ("GET", "/kg/read_graph"),
        "kg_tag_entity" => ("POST", "/kg/tag_entity"),
        "kg_get_tags" => ("GET", "/kg/get_tags"),
        "kg_remove_tag" => ("POST", "/kg/remove_tag"),
        "kg_delete_entity" => ("POST", "/kg/delete_entity"),
        "kg_delete_relation" => ("POST", "/kg/delete_relation"),
        // System (dot notation)
        "system.status" => ("GET", "/status"),
        "system.cleanup" => ("POST", "/system/cleanup"),
        "system.backup" => ("POST", "/system/backup"),
        "system.restore" => ("POST", "/system/restore"),
        // System (underscore notation)
        "system_status" => ("GET", "/status"),
        "system_cleanup" => ("POST", "/system/cleanup"),
        "system_backup" => ("POST", "/system/backup"),
        "system_restore" => ("POST", "/system/restore"),
        // Advanced (dot notation)
        "advanced.consolidate" => ("POST", "/advanced/consolidate"),
        "advanced.analyze_patterns" => ("POST", "/advanced/analyze_patterns"),
        "advanced.reindex" => ("POST", "/advanced/reindex"),
        "advanced.trends" => ("POST", "/advanced/trends"),
        "advanced.clusters" => ("POST", "/advanced/clusters"),
        "advanced.relationships" => ("POST", "/advanced/relationships"),
        "advanced.effectiveness" => ("POST", "/advanced/effectiveness"),
        // Advanced (underscore notation)
        "advanced_consolidate" => ("POST", "/advanced/consolidate"),
        "advanced_analyze_patterns" => ("POST", "/advanced/analyze_patterns"),
        "advanced_reindex" => ("POST", "/advanced/reindex"),
        "advanced_trends" => ("POST", "/advanced/trends"),
        "advanced_clusters" => ("POST", "/advanced/clusters"),
        "advanced_relationships" => ("POST", "/advanced/relationships"),
        "advanced_effectiveness" => ("POST", "/advanced/effectiveness"),
        _ => return Err(format!("Unknown tool: {}", tool_name)),
    };
    let url = format!("{}{}", base, path);
    let client = reqwest::Client::new();
    let resp_result = if method == "GET" {
        let mut qp: Vec<(String, String)> = Vec::new();
        if let Some(map) = args.as_object() {
            for (k, v) in map.iter() {
                let s = if v.is_string() || v.is_number() || v.is_boolean() { v.to_string().trim_matches('"').to_string() } else { v.to_string() };
                qp.push((k.clone(), s));
            }
        }
        client.get(&url).query(&qp).send().await
    } else {
        client.post(&url).json(args).send().await
    };
    match resp_result {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.is_success() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(val)
                } else {
                    Ok(serde_json::Value::String(text))
                }
            } else {
                Err(format!("HTTP {}: {}", status.as_u16(), text))
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn health() -> Json<Health> { Json(Health { status: "ok" }) }

async fn status(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Json<StatusResponse> {
    Json(build_status(state).await)
}

async fn build_status(state: Arc<AppState>) -> StatusResponse {
	let uptime_ms = state.start_time.elapsed().as_millis();
    // Indices
    let mut idx = IndicesStatus::default();
    if let Ok(tree) = state.db.open_tree("mem_embeddings") { idx.vector.items += tree.iter().count() as u64; }
    if let Ok(tree) = state.db.open_tree("embeddings") { idx.vector.items += tree.iter().count() as u64; }
    if let Ok(tree) = state.db.open_tree("text_index") { idx.text.docs = tree.iter().count() as u64; }
    if let Ok(tree) = state.db.open_tree("kg_nodes") { idx.graph.nodes = tree.iter().count() as u64; }
    if let Ok(tree) = state.db.open_tree("kg_edges") { idx.graph.edges = tree.iter().count() as u64; }
    // Storage
    let data_root = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let warm_mb = dir_size_mb(std::path::Path::new(&data_root).join("warm").as_path());
    let cold_mb = dir_size_mb(std::path::Path::new(&data_root).join("cold").as_path());
    let storage = StorageStatus { hot_mb: 0, warm_mb, cold_mb };
    let metrics = { state.metrics.lock().await.clone() };
    // Process memory and STM/LTM counts
    let mut pm = ProcMem::default();
    pm.rss_mb = current_process_rss_mb().unwrap_or(0);
    if let Ok(tree) = state.db.open_tree("memories") {
        let mut stm = 0u64; let mut ltm = 0u64;
        for kv in tree.iter() {
            if let Ok((_, v)) = kv {
                if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
                    if let Some(layer) = rec.get("layer").and_then(|x| x.as_str()) {
                        if layer == "STM" { stm += 1; } else if layer == "LTM" { ltm += 1; }
                    }
                }
            }
        }
        pm.stm_count = stm; pm.ltm_count = ltm;
    }
    let mut health = "ok";
    // Degrade if p95 too high or memory too large
    let p95_threshold = std::env::var("STATUS_P95_MS_THRESHOLD").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(250.0);
    let rss_threshold_mb = std::env::var("STATUS_RSS_MB_THRESHOLD").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(2048);
    if metrics.p95_ms > p95_threshold as f64 || pm.rss_mb > rss_threshold_mb { health = "degraded"; }
    StatusResponse { uptime_ms, indices: idx, storage, metrics, proc_mem: pm, health }
}

async fn metrics_route(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> (axum::http::StatusCode, String) {
    // Expose minimal Prometheus text format
    let metrics = { state.metrics.lock().await.clone() };
    let mut out = String::new();
    out.push_str("# TYPE mcp_queries_total counter\n");
    out.push_str(&format!("mcp_queries_total {}\n", metrics.count));
    out.push_str("# TYPE mcp_cache_hits_total counter\n");
    out.push_str(&format!("mcp_cache_hits_total {}\n", metrics.cache_hits));
    out.push_str("# TYPE mcp_cache_misses_total counter\n");
    out.push_str(&format!("mcp_cache_misses_total {}\n", metrics.cache_misses));
    out.push_str("# TYPE mcp_query_last_ms gauge\n");
    out.push_str(&format!("mcp_query_last_ms {}\n", metrics.last_ms));
    out.push_str("# TYPE mcp_query_avg_ms gauge\n");
    out.push_str(&format!("mcp_query_avg_ms {}\n", metrics.avg_ms));
    out.push_str("# TYPE mcp_query_p50_ms gauge\n");
    out.push_str(&format!("mcp_query_p50_ms {}\n", metrics.p50_ms));
    out.push_str("# TYPE mcp_query_p95_ms gauge\n");
    out.push_str(&format!("mcp_query_p95_ms {}\n", metrics.p95_ms));
    out.push_str("# TYPE mcp_query_qps_1m gauge\n");
    out.push_str(&format!("mcp_query_qps_1m {}\n", metrics.qps_1m));
    (axum::http::StatusCode::OK, out)
}

#[cfg(target_os = "windows")]
fn current_process_rss_mb() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let handle = GetCurrentProcess();
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let ok = K32GetProcessMemoryInfo(handle, &mut counters as *mut _ as _, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32);
        if ok != 0 { Some((counters.WorkingSetSize / (1024*1024)) as u64) } else { None }
    }
}

#[cfg(not(target_os = "windows"))]
fn current_process_rss_mb() -> Option<u64> {
    // Fallback: read /proc/self/statm on unix-like
    let data = std::fs::read_to_string("/proc/self/statm").ok()?;
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
    let parts: Vec<&str> = data.split_whitespace().collect();
    if parts.len() >= 2 {
        let rss_pages: u64 = parts[1].parse().ok()?;
        Some((rss_pages * page_size) / (1024*1024))
    } else { None }
}

fn list_tools() -> Vec<ToolDescriptor> {
	vec![
		ToolDescriptor { name: "memory.add", description: "Add a memory entry" },
		ToolDescriptor { name: "memory.search", description: "Hybrid search across indices" },
		ToolDescriptor { name: "memory.update", description: "Update a memory entry" },
		ToolDescriptor { name: "memory.delete", description: "Delete a memory entry" },
		ToolDescriptor { name: "document.store", description: "Ingest a document" },
		ToolDescriptor { name: "document.retrieve", description: "Retrieve a document" },
		ToolDescriptor { name: "document.analyze", description: "Analyze a document" },
		ToolDescriptor { name: "document.refs_for_memory", description: "List document references for a memory" },
		ToolDescriptor { name: "document.refs_for_document", description: "List memories referencing a document" },
		ToolDescriptor { name: "document.validate_refs", description: "Validate and fix documentary references" },
		ToolDescriptor { name: "kg.list_entities", description: "List top entities by mention count" },
		ToolDescriptor { name: "kg.get_entity", description: "Get detailed information about an entity" },
		ToolDescriptor { name: "kg.create_entity", description: "Create or ensure an entity node exists" },
		ToolDescriptor { name: "kg.create_relation", description: "Create a relation between two nodes" },
		ToolDescriptor { name: "kg.search_nodes", description: "Search nodes by type and pattern" },
		ToolDescriptor { name: "kg.read_graph", description: "Get graph snapshot with configurable limit" },
		ToolDescriptor { name: "kg.tag_entity", description: "Add tags to an entity" },
		ToolDescriptor { name: "kg.get_tags", description: "Get all tags or entities by tag" },
		ToolDescriptor { name: "kg.remove_tag", description: "Remove tags from an entity" },
		ToolDescriptor { name: "kg.delete_entity", description: "Delete an entity and its edges" },
		ToolDescriptor { name: "kg.delete_relation", description: "Delete a specific relation" },
		ToolDescriptor { name: "system.status", description: "Get system status" },
		ToolDescriptor { name: "system.cleanup", description: "Run cleanup tasks" },
		ToolDescriptor { name: "system.backup", description: "Create a backup" },
		ToolDescriptor { name: "system.restore", description: "Restore from backup" },
		ToolDescriptor { name: "advanced.consolidate", description: "Promote STM to LTM" },
		ToolDescriptor { name: "advanced.analyze_patterns", description: "Analyze memory patterns" },
		ToolDescriptor { name: "advanced.reindex", description: "Rebuild indices" },
        ToolDescriptor { name: "advanced.trends", description: "Temporal trends across memory layers" },
        ToolDescriptor { name: "advanced.clusters", description: "Cross-document clusters via RELATED edges" },
        ToolDescriptor { name: "advanced.relationships", description: "Relationship strength analysis in KG" },
        ToolDescriptor { name: "advanced.effectiveness", description: "Memory effectiveness scoring" },
	]
}

async fn list_tools_route() -> Json<Vec<ToolDescriptor>> { Json(list_tools()) }

async fn document_store(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(req): Json<StoreDocRequest>) -> Response {
    let _permit = state.ingest_sema.acquire().await.expect("sema");
	let mime = req.mime.unwrap_or_else(|| "md".to_string());
	let content = if let Some(c) = req.content {
		c
	} else if let Some(path) = req.path.clone() {
        if (mime == "pdf") || path.to_lowercase().ends_with(".pdf") {
            match read_pdf_text(&path) {
                Ok(t) => t,
                Err(_) => return json_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Failed to read PDF from path", Some(serde_json::json!({"path": path})))
            }
		} else {
            match std::fs::read_to_string(&path) {
                Ok(raw) => { if mime == "md" || path.to_lowercase().ends_with(".md") { markdown_to_text(&raw) } else { raw } },
                Err(_) => return json_error(StatusCode::NOT_FOUND, "NOT_FOUND", "File not found", Some(serde_json::json!({"path": path})))
            }
		}
	} else {
        return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "Provide either content or path", None);
	};
	let mut hasher = Sha256::new();
	hasher.update(content.as_bytes());
	let hash = format!("{:x}", hasher.finalize());

    // Trees used for documents and versioning
    let docs = state.db.open_tree("docs").expect("docs tree"); // hash -> id
    let docs_info = state.db.open_tree("docs_info").expect("docs_info tree"); // id -> {path, hash, version, prev_id, created_at}
    let path_latest = state.db.open_tree("doc_path_latest").expect("path latest tree"); // path -> id
    let versions = state.db.open_tree("doc_versions").expect("doc versions tree"); // path:version -> id

	// Dedup: check docs tree by hash
	if let Ok(Some(existing)) = docs.get(hash.as_bytes()) {
		let id = String::from_utf8(existing.to_vec()).unwrap_or_else(|_| Uuid::new_v4().to_string());
        // If a path is provided, ensure version mappings exist
        if let Some(ref p) = req.path {
            let prev_id = path_latest.get(p.as_bytes()).ok().flatten().map(|v| String::from_utf8(v.to_vec()).unwrap_or_default());
            let prev_version = prev_id.as_ref().and_then(|pid| {
                docs_info.get(pid.as_bytes()).ok().flatten().and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok()).and_then(|v| v.get("version").and_then(|n| n.as_u64()))
            }).unwrap_or(0);
            let ver = if prev_id.as_deref() == Some(&id) { prev_version } else { prev_version + 1 };
            // Upsert docs_info for id if missing
            if docs_info.get(id.as_bytes()).ok().flatten().is_none() {
                let info = serde_json::json!({"path": p, "hash": hash, "version": ver, "prev_id": prev_id, "created_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() });
                let _ = docs_info.insert(id.as_bytes(), serde_json::to_vec(&info).unwrap());
            }
            let _ = path_latest.insert(p.as_bytes(), id.as_bytes());
            let ver_key = format!("{}:{}", p, ver);
            let _ = versions.insert(ver_key.as_bytes(), id.as_bytes());
        }
        return Json(StoreDocResponse { id, hash, chunks: 0 }).into_response();
	}

	let id = Uuid::new_v4().to_string();
	docs.insert(hash.as_bytes(), id.as_bytes()).expect("insert doc");
	// Persist minimal metadata so request.metadata is used and not warned
	if let Some(meta) = req.metadata {
		let meta_tree = state.db.open_tree("docs_meta").expect("docs_meta tree");
		let key = format!("{}:meta", id);
		let val = serde_json::to_vec(&meta).unwrap_or_else(|_| b"{}".to_vec());
		let _ = meta_tree.insert(key.as_bytes(), val);
	}
    // Versioning if path is provided
    if let Some(ref p) = req.path {
        let prev_id = path_latest.get(p.as_bytes()).ok().flatten().map(|v| String::from_utf8(v.to_vec()).unwrap_or_default());
        let prev_version = prev_id.as_ref().and_then(|pid| {
            docs_info.get(pid.as_bytes()).ok().flatten().and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok()).and_then(|v| v.get("version").and_then(|n| n.as_u64()))
        }).unwrap_or(0);
        let ver = prev_version + 1;
        let info = serde_json::json!({"path": p, "hash": hash, "version": ver, "prev_id": prev_id, "created_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() });
        let _ = docs_info.insert(id.as_bytes(), serde_json::to_vec(&info).unwrap());
        let _ = path_latest.insert(p.as_bytes(), id.as_bytes());
        let ver_key = format!("{}:{}", p, ver);
        let _ = versions.insert(ver_key.as_bytes(), id.as_bytes());
	}
	let chunks = chunk_markdown(&content);
	let chunks_tree = state.db.open_tree("chunks").expect("chunks tree");
	for ch in &chunks {
		let key = format!("{}:{}", id, ch.position.start);
		let val = serde_json::to_vec(ch).unwrap();
		chunks_tree.insert(key.as_bytes(), val).expect("insert chunk");
	}
	// batch embed placeholders and persist
	let emb_tree = state.db.open_tree("embeddings").expect("embeddings tree");
	let texts: Vec<&str> = chunks.iter().map(|_| "").collect();
	let vecs = embeddings::embed_batch(&texts);
	for (idx, ch) in chunks.iter().enumerate() {
		let key = format!("{}:{}", id, ch.position.start);
		let bytes: &[u8] = bytemuck::cast_slice(&vecs[idx]);
		emb_tree.insert(key.as_bytes(), bytes).expect("insert emb");
	}
	// update vector index scaffold metadata
	let starts: Vec<usize> = chunks.iter().map(|c| c.position.start).collect();
	vector_index::record_vectors(&state.db, &id, &starts, embeddings::EMBED_DIM).expect("vec meta");
	// extract and link entities (basic heuristic)
	let entities = kg::extract_entities(&content);
	kg::link_entities(&state.db, &id, &entities).expect("kg link");
	let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	kg::ensure_document_node(&state.db, &id, now_ms).ok();
	for e in &entities { kg::ensure_entity_node(&state.db, e, now_ms).ok(); kg::add_edge(&state.db, e, &id, "MENTIONS", now_ms).ok(); }
	// Relate to existing documents by shared entities (best-effort)
	if let Ok(existing) = state.db.open_tree("doc_path_latest") { // iterate latest known docs
		for kv in existing.iter() {
			if let Ok((_, v)) = kv { if let Ok(other_id) = String::from_utf8(v.to_vec()) { if other_id != id { kg::relate_documents_by_entities(&state.db, &id, &other_id, now_ms).ok(); } } }
		}
	}
	index_chunks_tantivy(&state.index_dir, &id, &chunks, &content).expect("index tantivy");
	index_chunks_sled(&state.db, &id, &chunks, &content).expect("index text");
	state.db.flush().expect("flush");
    Json(StoreDocResponse { id, hash, chunks: chunks.len() }).into_response()
}

async fn document_retrieve(axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>, axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Response {
	let id = params.get("id").cloned();
	let hash = params.get("hash").cloned();
    let path = params.get("path").cloned();
    if id.is_none() && hash.is_none() && path.is_none() {
        return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "Specify id, hash, or path", None);
    }
	let docs = state.db.open_tree("docs").expect("docs tree");
	let resolved_id = if let Some(h) = hash { 
		match docs.get(h.as_bytes()) { Ok(Some(v)) => String::from_utf8(v.to_vec()).unwrap_or_default(), _ => String::new() }
    } else if let Some(p) = path {
        let path_latest = state.db.open_tree("doc_path_latest").expect("path latest tree");
        match path_latest.get(p.as_bytes()) { Ok(Some(v)) => String::from_utf8(v.to_vec()).unwrap_or_default(), _ => String::new() }
	} else { id.unwrap_or_default() };
    if resolved_id.is_empty() { return json_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Document not found", None); }
	let chunks_tree = state.db.open_tree("chunks").expect("chunks tree");
	let prefix = format!("{}:", resolved_id);
	let mut chunks: Vec<ChunkHeader> = Vec::new();
	for item in chunks_tree.scan_prefix(prefix.as_bytes()) { if let Ok((_, v)) = item { if let Ok(ch) = serde_json::from_slice::<ChunkHeader>(&v) { chunks.push(ch) } } }
    // Include metadata if present
    let meta_tree = state.db.open_tree("docs_meta").expect("docs_meta tree");
    let meta_key = format!("{}:meta", resolved_id);
    let metadata = meta_tree.get(meta_key.as_bytes()).ok().flatten().and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok());
    if chunks.is_empty() { return json_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Document not found", None); }
    Json(serde_json::json!({ "id": resolved_id, "chunks": chunks, "metadata": metadata })).into_response()
}

fn chunk_markdown(content: &str) -> Vec<ChunkHeader> {
	let max_len = 1000usize;
	let mut chunks = Vec::new();
	let mut start = 0usize;
	while start < content.len() {
		let end = (start + max_len).min(content.len());
		let id = Uuid::new_v4().to_string();
		chunks.push(ChunkHeader { id, position: Position { start, end } });
		start = end;
	}
	chunks
}

fn read_pdf_text(path: &str) -> Result<String> {
	let doc = LoDocument::load(path)?;
	let mut out = String::new();
    // Limits for large PDFs (best-effort streaming-like behavior)
    let max_pages: usize = std::env::var("PDF_MAX_PAGES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let max_bytes: usize = std::env::var("PDF_MAX_BYTES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let max_time_ms: u128 = std::env::var("PDF_MAX_TIME_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let started = Instant::now();
    let mut page_count: usize = 0;
    let mut stop = false;
	for page_id in doc.get_pages().values() {
        if stop { break; }
        if max_pages > 0 && page_count >= max_pages { break; }
        if max_time_ms > 0 && started.elapsed().as_millis() >= max_time_ms { break; }
        page_count += 1;
        let page = LoDocument::get_page_content(&doc, *page_id)?;
		let content = lopdf::content::Content::decode(&page)?;
		for operation in content.operations {
            if stop { break; }
			if operation.operator == "Tj" || operation.operator == "TJ" {
				for operand in operation.operands {
					if let lopdf::Object::String(s, _) = operand {
						let bytes: Vec<u8> = s.into();
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            out.push_str(text);
                            out.push('\n');
                            if max_bytes > 0 && out.len() >= max_bytes { stop = true; break; }
                            if max_time_ms > 0 && started.elapsed().as_millis() >= max_time_ms { stop = true; break; }
                        }
					}
				}
			}
		}
	}
	Ok(out)
}


fn markdown_to_text(md: &str) -> String {
	let mut out = String::new();
	let parser = MdParser::new_ext(md, MdOptions::ENABLE_STRIKETHROUGH | MdOptions::ENABLE_TABLES);
	for event in parser {
		match event { MdEvent::Text(t) => { out.push_str(&t); }, MdEvent::SoftBreak | MdEvent::HardBreak => out.push('\n'), _ => {} }
	}
	out
}

async fn document_analyze(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<serde_json::Value> {
	let id = params.get("id").cloned().unwrap_or_default();
	let entities = kg::entities_for_doc(&state.db, &id).unwrap_or_default();
    // Derive simple key concepts as top frequent entities
    let key_concepts = entities.iter().take(5).cloned().collect::<Vec<_>>();
    // Compose a trivial summary from first chunk
    let chunks_tree = state.db.open_tree("chunks").expect("chunks tree");
    let prefix = format!("{}:", id);
    let mut first_text: Option<String> = None;
    for item in chunks_tree.scan_prefix(prefix.as_bytes()).take(1) { if let Ok((k,_)) = item { let key = String::from_utf8(k.to_vec()).unwrap_or_default(); if let Some((_, _start_str)) = key.split_once(":") { let idx = state.db.open_tree("text_index").expect("text_index"); if let Ok(Some(v)) = idx.get(key.as_bytes()) { first_text = Some(String::from_utf8_lossy(&v).chars().take(300).collect()); } } } }
    let summary = first_text;
    // Collect related documents from KG
    let mut related: Vec<serde_json::Value> = Vec::new();
    if let Ok(edges) = state.db.open_tree("kg_edges") {
        let src = format!("Document::{}", id);
        let prefix = format!("{}->", src);
        for kv in edges.scan_prefix(prefix.as_bytes()) {
            if let Ok((k, v)) = kv {
                let key = String::from_utf8(k.to_vec()).unwrap_or_default();
                if key.ends_with("::RELATED") {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&v) {
                        if let Some(dst) = val.get("dst").and_then(|x| x.as_str()) { related.push(serde_json::json!({ "docId": dst.strip_prefix("Document::").unwrap_or(dst), "score": val.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) })); }
                    }
                }
            }
        }
    }
    Json(serde_json::json!({ "id": id, "keyConcepts": key_concepts, "entities": entities, "summary": summary, "docRefs": related }))
}

async fn kg_entities(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Json<serde_json::Value> {
	let list = kg::list_entities(&state.db, 50).unwrap_or_default();
	Json(serde_json::json!({ "entities": list }))
}

async fn kg_docs_for_entity(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<serde_json::Value> {
	let entity = params.get("entity").cloned().unwrap_or_default();
	let docs = kg::docs_for_entity(&state.db, &entity).unwrap_or_default();
	Json(serde_json::json!({ "entity": entity, "docs": docs }))
}

async fn kg_snapshot(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Json<serde_json::Value> {
	use petgraph::graph::Graph;
	let ents = kg::list_entities(&state.db, 100).unwrap_or_default();
	let mut g: Graph<String, String> = Graph::new();
	let mut nodes = std::collections::HashMap::new();
	for (e, _) in &ents { let n = g.add_node(e.clone()); nodes.insert(e.clone(), n); }
	for (e, _) in &ents {
		let docs = kg::docs_for_entity(&state.db, e).unwrap_or_default();
		for d in docs {
			let doc_node = nodes.entry(d.clone()).or_insert_with(|| g.add_node(d.clone())).to_owned();
			let e_node = nodes.get(e).cloned().unwrap();
			let _ = g.add_edge(e_node, doc_node, "MENTIONS".to_string());
		}
	}
	let nodes_out: Vec<String> = g.node_indices().map(|i| g[i].clone()).collect();
	let edges_out: Vec<(String,String,String)> = g.edge_indices().map(|eidx| {
		let (s,t) = g.edge_endpoints(eidx).unwrap();
		(g[s].clone(), g[t].clone(), g[eidx].clone())
	}).collect();
	Json(serde_json::json!({ "nodes": nodes_out, "edges": edges_out }))
}

async fn kg_list_entities(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<serde_json::Value> {
	let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
	let list = kg::list_entities(&state.db, limit).unwrap_or_default();
	Json(serde_json::json!({ "entities": list }))
}

async fn kg_get_entity(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
	let entity = match params.get("entity").cloned() {
		Some(e) => e,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "entity parameter required", None)
	};
	match kg::get_entity_details(&state.db, &entity) {
		Ok(details) => Json(details).into_response(),
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

async fn kg_create_entity(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
	let entity = match body.get("entity").and_then(|e| e.as_str()) {
		Some(e) => e,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "entity field required", None)
	};
	let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	match kg::ensure_entity_node(&state.db, entity, now_ms) {
		Ok(_) => {
			state.db.flush().ok();
			Json(serde_json::json!({ "entity": entity, "created": true })).into_response()
		}
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

async fn kg_create_relation(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
	let src = match body.get("src").and_then(|s| s.as_str()) {
		Some(s) => s,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "src field required", None)
	};
	let dst = match body.get("dst").and_then(|d| d.as_str()) {
		Some(d) => d,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "dst field required", None)
	};
	let relation = body.get("relation").and_then(|r| r.as_str()).unwrap_or("RELATED");
	let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	
	match kg::add_edge_generic(&state.db, src, dst, relation, now_ms) {
		Ok(_) => {
			state.db.flush().ok();
			Json(serde_json::json!({ "src": src, "dst": dst, "relation": relation, "created": true })).into_response()
		}
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

async fn kg_search_nodes(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<serde_json::Value> {
	let node_type = params.get("type").map(|s| s.as_str());
	let pattern = params.get("pattern").map(|s| s.as_str());
	let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
	let results = kg::search_nodes(&state.db, node_type, pattern, limit).unwrap_or_default();
	Json(serde_json::json!({ "nodes": results, "count": results.len() }))
}

async fn kg_read_graph(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<serde_json::Value> {
	let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(100);
	// Similar to kg_snapshot but with configurable limit
	use petgraph::graph::Graph;
	let ents = kg::list_entities(&state.db, limit).unwrap_or_default();
	let mut g: Graph<String, String> = Graph::new();
	let mut nodes = std::collections::HashMap::new();
	for (e, _) in &ents { let n = g.add_node(e.clone()); nodes.insert(e.clone(), n); }
	for (e, _) in &ents {
		let docs = kg::docs_for_entity(&state.db, e).unwrap_or_default();
		for d in docs {
			let doc_node = nodes.entry(d.clone()).or_insert_with(|| g.add_node(d.clone())).to_owned();
			let e_node = nodes.get(e).cloned().unwrap();
			let _ = g.add_edge(e_node, doc_node, "MENTIONS".to_string());
		}
	}
	let nodes_out: Vec<String> = g.node_indices().map(|i| g[i].clone()).collect();
	let edges_out: Vec<(String,String,String)> = g.edge_indices().map(|eidx| {
		let (s,t) = g.edge_endpoints(eidx).unwrap();
		(g[s].clone(), g[t].clone(), g[eidx].clone())
	}).collect();
	Json(serde_json::json!({ "nodes": nodes_out, "edges": edges_out }))
}

async fn kg_tag_entity(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
	let entity = match body.get("entity").and_then(|e| e.as_str()) {
		Some(e) => e,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "entity field required", None)
	};
	
	// Accept tags as either array or comma-separated string for flexibility
	let tags: Vec<String> = match body.get("tags") {
		Some(val) => {
			if let Some(arr) = val.as_array() {
				// Normal case: array of strings
				arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
			} else if let Some(s) = val.as_str() {
				// Fallback: comma-separated string
				s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
			} else {
				return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", 
					format!("tags must be array or string, got: {:?}", val), None)
			}
		}
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "tags field required", None)
	};
	
	if tags.is_empty() {
		return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "tags cannot be empty", None);
	}
	
	match kg::tag_entity(&state.db, entity, &tags) {
		Ok(_) => {
			state.db.flush().ok();
			Json(serde_json::json!({ "entity": entity, "tags": tags, "tagged": true })).into_response()
		}
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

async fn kg_get_tags(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<serde_json::Value> {
	if let Some(tag) = params.get("tag") {
		// Get entities by specific tag
		let entities = kg::get_entities_by_tag(&state.db, tag).unwrap_or_default();
		Json(serde_json::json!({ "tag": tag, "entities": entities }))
	} else {
		// Get all tags
		let tags = kg::get_all_tags(&state.db).unwrap_or_default();
		Json(serde_json::json!({ "tags": tags }))
	}
}

async fn kg_remove_tag(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
	let entity = match body.get("entity").and_then(|e| e.as_str()) {
		Some(e) => e,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "entity field required", None)
	};
	
	// Accept tags as either array or comma-separated string for flexibility
	let tags: Vec<String> = match body.get("tags") {
		Some(val) => {
			if let Some(arr) = val.as_array() {
				// Normal case: array of strings
				arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
			} else if let Some(s) = val.as_str() {
				// Fallback: comma-separated string
				s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
			} else {
				return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", 
					format!("tags must be array or string, got: {:?}", val), None)
			}
		}
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "tags field required", None)
	};
	
	if tags.is_empty() {
		return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "tags cannot be empty", None);
	}
	
	match kg::remove_tags_from_entity(&state.db, entity, &tags) {
		Ok(_) => {
			state.db.flush().ok();
			Json(serde_json::json!({ "entity": entity, "removed": tags, "success": true })).into_response()
		}
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

async fn kg_delete_entity(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
	let entity = match body.get("entity").and_then(|e| e.as_str()) {
		Some(e) => e,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "entity field required", None)
	};
	
	match kg::delete_entity(&state.db, entity) {
		Ok(removed) => {
			state.db.flush().ok();
			Json(serde_json::json!({ "entity": entity, "deleted": true, "removedItems": removed })).into_response()
		}
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

async fn kg_delete_relation(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
	let src = match body.get("src").and_then(|s| s.as_str()) {
		Some(s) => s,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "src field required", None)
	};
	let dst = match body.get("dst").and_then(|d| d.as_str()) {
		Some(d) => d,
		None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "dst field required", None)
	};
	let relation = body.get("relation").and_then(|r| r.as_str()).unwrap_or("RELATED");
	
	match kg::delete_relation(&state.db, src, dst, relation) {
		Ok(deleted) => {
			state.db.flush().ok();
			Json(serde_json::json!({ "src": src, "dst": dst, "relation": relation, "deleted": deleted })).into_response()
		}
		Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
	}
}

fn index_chunks_tantivy(index_dir: &std::path::Path, doc_id: &str, chunks: &[ChunkHeader], full_text: &str) -> Result<()> {
	use tantivy::{schema::*, Index, doc, directory::MmapDirectory};
	let mut schema_builder = Schema::builder();
	let id_f = schema_builder.add_text_field("id", TEXT | STORED);
	let t_f = schema_builder.add_text_field("type", STRING | STORED);
	let content_f = schema_builder.add_text_field("content", TEXT);
	let ts_f = schema_builder.add_i64_field("timestamp", INDEXED);
	let schema = schema_builder.build();
	let dir = index_dir.join("tantivy");
	std::fs::create_dir_all(&dir)?;
	let directory = MmapDirectory::open(&dir)?;
	let index = Index::open_or_create(directory, schema.clone())?;
	let mut writer = index.writer(50_000_000)?;
	let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	for ch in chunks {
		let start = ch.position.start;
		let end = ch.position.end.min(full_text.len());
		let text_slice = &full_text[start..end];
		let _ = writer.add_document(doc!(id_f=>format!("{}:{}", doc_id, start), t_f=>"chunk", content_f=>text_slice, ts_f=>now));
	}
	writer.commit()?;
	Ok(())
}

fn index_memory_tantivy(index_dir: &std::path::Path, mem_id: &str, content: &str) -> Result<()> {
    use tantivy::{schema::*, Index, doc, directory::MmapDirectory};
    let mut schema_builder = Schema::builder();
    let id_f = schema_builder.add_text_field("id", TEXT | STORED);
    let t_f = schema_builder.add_text_field("type", STRING | STORED);
    let content_f = schema_builder.add_text_field("content", TEXT);
    let ts_f = schema_builder.add_i64_field("timestamp", INDEXED);
    let schema = schema_builder.build();
    let dir = index_dir.join("tantivy");
    std::fs::create_dir_all(&dir)?;
    let directory = MmapDirectory::open(&dir)?;
    let index = Index::open_or_create(directory, schema.clone())?;
    let mut writer = index.writer(50_000_000)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let _ = writer.add_document(doc!(id_f=>format!("mem:{}", mem_id), t_f=>"memory", content_f=>content, ts_f=>now));
    writer.commit()?;
    Ok(())
}

fn index_memory_sled(db: &sled::Db, mem_id: &str, content: &str) -> Result<()> {
    let text_idx = db.open_tree("text_index")?;
    let key = format!("mem:{}", mem_id);
    text_idx.insert(key.as_bytes(), content.as_bytes())?;
	Ok(())
}

async fn run_stdio(_state: Arc<AppState>) {
	let stdin = tokio::io::stdin();
	let mut reader = BufReader::new(stdin).lines();
	while let Ok(Some(line)) = reader.next_line().await {
		let line = line.trim();
		if line.is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(line) { Ok(x) => x, Err(_) => continue };
        let id_val_opt = v.get("id").cloned();
        let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));
        // Ignore notifications (no id), including notifications/initialized
        if id_val_opt.is_none() || id_val_opt.as_ref().map(|x| x.is_null()).unwrap_or(true) {
            // Optionally handle side-effects for known notifications here
            continue;
        }
        let id_val = id_val_opt.unwrap();
        match method {
            "initialize" => {
                let result = serde_json::json!({
                    "serverInfo": {
                        "name": "memorized-mcp",
                        "version": env!("CARGO_PKG_VERSION"),
                        "instructions": "MemorizedMCP: hybrid memory server exposing tools over MCP."
                    },
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": { "listChanged": true, "call": {} }, "logging": {}, "sampling": {} }
                });
                let mut out = serde_json::json!({ "jsonrpc": "2.0", "id": serde_json::Value::Null });
                out["id"] = id_val.clone();
                out["result"] = result;
                println!("{}", serde_json::to_string(&out).unwrap());
            }
            "tools/list" => {
                let tools = list_tools().into_iter().map(|t| serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": { "type": "object", "properties": {}, "additionalProperties": true }
                })).collect::<Vec<_>>();
                let mut out = serde_json::json!({ "jsonrpc": "2.0", "id": serde_json::Value::Null });
                out["id"] = id_val.clone();
                out["result"] = serde_json::json!({ "tools": tools });
                println!("{}", serde_json::to_string(&out).unwrap());
            }
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                let mut out = serde_json::json!({ "jsonrpc": "2.0", "id": serde_json::Value::Null });
                out["id"] = id_val.clone();
                match proxy_tool_via_http(name, &arguments).await {
                    Ok(json_val) => {
                        let text_payload = if let Some(s) = json_val.as_str() {
                            s.to_string()
                        } else {
                            serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| json_val.to_string())
                        };
                        out["result"] = serde_json::json!({ "content": [ { "type": "text", "text": text_payload } ] });
                    }
                    Err(err) => {
                        out["error"] = serde_json::json!({ "code": -32000, "message": err });
                    }
                }
                println!("{}", serde_json::to_string(&out).unwrap());
            }
            _ => {
                let mut out = serde_json::json!({ "jsonrpc": "2.0", "id": serde_json::Value::Null });
                out["id"] = id_val;
                out["error"] = serde_json::json!({ "code": -32601, "message": format!("Unknown method: {}", method) });
                println!("{}", serde_json::to_string(&out).unwrap());
            }
        }
    }
}

async fn memory_add(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(req): Json<AddMemoryRequest>) -> Response {
	let id = Uuid::new_v4().to_string();
	let layer = req.layer_hint.unwrap_or_else(|| "STM".to_string());
    if req.content.trim().is_empty() { return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "content must not be empty", None); }
	let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	let expires_at = if layer == "STM" { Some(now_ms + 60 * 60 * 1000) } else { None };
	let tree = state.db.open_tree("memories").expect("mem tree");

    // Create KG node for this memory and link any referenced documents as EVIDENCE
    kg::ensure_memory_node(&state.db, &id, now_ms).ok();
    // Semantic: link memory to mentioned entities
    let mem_ents_vec = kg::extract_entities(&req.content);
    for e in &mem_ents_vec { kg::ensure_entity_node(&state.db, e, now_ms).ok(); }
    for e in &mem_ents_vec {
        let src = format!("Memory::{}", &id);
        let dst = format!("Entity::{}", e);
        kg::add_edge_generic(&state.db, &src, &dst, "MENTIONS", now_ms).ok();
    }
    if let Some(ep) = req.episode_id.as_ref() {
        kg::ensure_episode_node(&state.db, ep, now_ms, None, req.session_id.as_deref()).ok();
        let src = format!("Memory::{}", &id);
        let dst = format!("Episode::{}", ep);
        kg::add_edge_generic(&state.db, &src, &dst, "IN_EPISODE", now_ms).ok();
    }
    let mut computed_refs: Option<Vec<serde_json::Value>> = None;
    if let Some(refs) = req.references.as_ref() {
        let mem_ents: HashSet<String> = mem_ents_vec.into_iter().collect();
        let mut out = Vec::new();
        for r in refs {
            let doc_id = &r.doc_id;
            kg::ensure_document_node(&state.db, doc_id, now_ms).ok();
            let src = format!("Memory::{}", &id);
            let dst = format!("Document::{}", doc_id);
            kg::add_edge_generic(&state.db, &src, &dst, "EVIDENCE", now_ms).ok();
            // Score evidence using Jaccard of entities if score not provided
            let doc_ents_vec = kg::entities_for_doc(&state.db, doc_id).unwrap_or_default();
            let doc_ents: HashSet<String> = doc_ents_vec.into_iter().collect();
            let inter = mem_ents.intersection(&doc_ents).count() as f32;
            let uni = mem_ents.union(&doc_ents).count() as f32;
            let jacc = if uni > 0.0 { inter / uni } else { 0.0 };
            let score = r.score.unwrap_or(jacc);
            out.push(serde_json::json!({ "docId": doc_id, "chunkId": r.chunk_id, "score": score }));
            // Persist in doc_refs tree
            if let Ok(tree_refs) = state.db.open_tree("doc_refs") {
                let key = format!("mem::{}::doc::{}::chunk::{}", id, doc_id, r.chunk_id.clone().unwrap_or_default());
                let _ = tree_refs.insert(key.as_bytes(), serde_json::to_vec(&serde_json::json!({"score": score})) .unwrap());
            }
        }
        computed_refs = Some(out);
    }
	let rec = serde_json::json!({
		"id": id,
		"content": req.content,
		"metadata": req.metadata,
		"layer": layer,
		"session_id": req.session_id,
		"episode_id": req.episode_id,
		"created_at": now_ms,
		"expires_at": expires_at,
		"docRefs": computed_refs
	});
	tree.insert(id.as_bytes(), serde_json::to_vec(&rec).unwrap()).expect("insert mem");
	// Reusable text index for memory (sled) and tantivy
	index_memory_sled(&state.db, &id, &rec.get("content").and_then(|c| c.as_str()).unwrap_or("")).ok();
	index_memory_tantivy(&state.index_dir, &id, rec.get("content").and_then(|c| c.as_str()).unwrap_or("")) .ok();
	// Store embedding for memory content (placeholder if feature not enabled)
	{
		let emb_tree = state.db.open_tree("mem_embeddings").expect("mem_embeddings");
		let vecs = embeddings::embed_batch(&[rec.get("content").and_then(|c| c.as_str()).unwrap_or("")]);
		let bytes: &[u8] = bytemuck::cast_slice(&vecs[0]);
		let _ = emb_tree.insert(id.as_bytes(), bytes);
	}
	state.db.flush().expect("flush");
    Json(AddMemoryResponse { id, layer }).into_response()
}

async fn memory_search(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<SearchResponse> {
    let started = std::time::Instant::now();
    let original_q = params.get("q").cloned().unwrap_or_default();
    let query = original_q.to_lowercase();
    let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
	let layer = params.get("layer").cloned();
	let episode = params.get("episode").cloned();
	let time_from = params.get("from").and_then(|s| s.parse::<i64>().ok());
	let time_to = params.get("to").and_then(|s| s.parse::<i64>().ok());
	let tree = state.db.open_tree("memories").expect("mem tree");
    let mut results: Vec<SearchResult> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
	let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	let strengthen_mul: f64 = std::env::var("LTM_STRENGTHEN_ON_ACCESS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.05);
	let stm_strengthen_add: f64 = std::env::var("STM_STRENGTHEN_DELTA").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
	for kv in tree.iter() {
		let (_, v) = kv.expect("ok");
		if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
			let content = rec.get("content").and_then(|c| c.as_str()).unwrap_or("").to_lowercase();
			let layer_v = rec.get("layer").and_then(|c| c.as_str()).unwrap_or("").to_string();
			let created_at = rec.get("created_at").and_then(|c| c.as_i64());
			let episode_v = rec.get("episode_id").and_then(|c| c.as_str());
			let in_time = created_at.map(|t| time_from.map(|f| t>=f).unwrap_or(true) && time_to.map(|to| t<=to).unwrap_or(true)).unwrap_or(true);
			let episode_ok = episode.as_deref().map(|e| Some(e)==episode_v).unwrap_or(true);
			if content.contains(&query) && layer.as_deref().map(|l| l==layer_v).unwrap_or(true) && in_time && episode_ok {
				let id = rec.get("id").and_then(|c| c.as_str()).unwrap_or("").to_string();
                if !seen.contains(&id) {
                    let doc_refs = rec.get("docRefs").and_then(|r| r.as_array()).map(|arr| {
                        arr.iter().filter_map(|x| {
                            let doc_id = x.get("docId").and_then(|v| v.as_str())?.to_string();
                            let chunk_id = x.get("chunkId").and_then(|v| v.as_str()).map(|s| s.to_string());
                            let score = x.get("score").and_then(|v| v.as_f64()).map(|f| f as f32);
                            Some(DocRefOut{ doc_id, chunk_id, score })
                        }).collect::<Vec<_>>()
                    });
                    results.push(SearchResult { id: id.clone(), score: 1.0, layer: layer_v.clone(), doc_refs, explain: None });
                    seen.insert(id.clone());
                }
				// Access-based strengthening and stats bump
				if let Ok(Some(old)) = tree.get(id.as_bytes()) {
					let mut r: serde_json::Value = serde_json::from_slice(&old).unwrap_or(serde_json::json!({}));
					let acc = r.get("access_count").and_then(|c| c.as_u64()).unwrap_or(0) + 1;
					r["access_count"] = serde_json::json!(acc);
					r["last_access_ts"] = serde_json::json!(now_ms);
					let imp = r.get("importance").and_then(|c| c.as_f64()).unwrap_or(1.0);
					let new_imp = if layer_v == "LTM" { imp * strengthen_mul } else { imp + stm_strengthen_add };
					r["importance"] = serde_json::json!(new_imp);
					let _ = tree.insert(id.as_bytes(), serde_json::to_vec(&r).unwrap());
				}
			}
		}
	}
    // Vector: embed query and search over memory embeddings (placeholder when no model)
    if !query.is_empty() {
        let qvec = embeddings::embed_batch(&[query.as_str()]);
        if let Some(vec) = qvec.get(0) {
            let topk = vector_index::search_memories_by_vector(&state.db, vec, limit);
            for (id, score) in topk {
                if !seen.contains(&id) {
                    results.push(SearchResult { id: id.clone(), score, layer: "LTM".to_string(), doc_refs: None, explain: Some(serde_json::json!({"source":"vector"})) });
                    seen.insert(id);
                }
            }
        }
    }
    Json(SearchResponse { results, took_ms: Some(started.elapsed().as_millis()) })
}

async fn memory_update(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(req): Json<UpdateMemoryRequest>) -> Response {
	let tree = state.db.open_tree("memories").expect("mem tree");
	if let Some(rec_v) = tree.get(req.id.as_bytes()).expect("get").map(|v| v.to_vec()) {
		let mut rec: JsonValue = serde_json::from_slice(&rec_v).unwrap_or(serde_json::json!({}));
        let mut reembed = false;
        if let Some(c) = req.content { rec["content"] = serde_json::json!(c); reembed = true; }
		if let Some(m) = req.metadata { rec["metadata"] = m; }
        let ver = rec.get("version").and_then(|v| v.as_u64()).unwrap_or(0) + 1;
        rec["version"] = serde_json::json!(ver);
		tree.insert(req.id.as_bytes(), serde_json::to_vec(&rec).unwrap()).expect("insert");
        // Re-embed and refresh indices on content change
        if reembed {
            let content = rec.get("content").and_then(|c| c.as_str()).unwrap_or("");
            // Update memory embedding
            if let Ok(emb_tree) = state.db.open_tree("mem_embeddings") {
                let vecs = embeddings::embed_batch(&[content]);
                let bytes: &[u8] = bytemuck::cast_slice(&vecs[0]);
                let _ = emb_tree.insert(req.id.as_bytes(), bytes);
            }
            // Refresh text indices
            let _ = index_memory_sled(&state.db, &req.id, content);
            let _ = index_memory_tantivy(&state.index_dir, &req.id, content);
        }
		state.db.flush().expect("flush");
        Json(serde_json::json!({ "id": req.id, "version": ver, "reembedded": reembed, "updatedIndices": ["text", "vector"] })).into_response()
	} else {
        json_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Memory not found", None)
	}
}

async fn memory_delete(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(req): Json<DeleteMemoryRequest>) -> Response {
	let tree = state.db.open_tree("memories").expect("mem tree");
    // Optional backup
    if req.backup.unwrap_or(false) {
        if let Ok(Some(v)) = tree.get(req.id.as_bytes()) {
            if let Ok(backup) = state.db.open_tree("backups_memories") {
                let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
                let key = format!("{}:{}", ts, &req.id);
                let _ = backup.insert(key.as_bytes(), v);
            }
        }
    }
    // Dependency checks: remove KG edges from/to this memory; doc_refs
    if let Ok(edges) = state.db.open_tree("kg_edges") {
        let prefix = format!("Memory::{}->", &req.id);
        let to_remove: Vec<_> = edges.scan_prefix(prefix.as_bytes()).filter_map(|kv| kv.ok().map(|(k, _)| k)).collect();
        for k in to_remove { let _ = edges.remove(k); }
    }
    if let Ok(text_idx) = state.db.open_tree("text_index") { let _ = text_idx.remove(format!("mem:{}", &req.id).as_bytes()); }
    if let Ok(emb) = state.db.open_tree("mem_embeddings") { let _ = emb.remove(req.id.as_bytes()); }
    if let Ok(refs) = state.db.open_tree("doc_refs") {
        let prefix = format!("mem::{}::", &req.id);
        let to_remove: Vec<_> = refs.scan_prefix(prefix.as_bytes()).filter_map(|kv| kv.ok().map(|(k, _)| k)).collect();
        for k in to_remove { let _ = refs.remove(k); }
    }
	let existed = tree.remove(req.id.as_bytes()).expect("remove").is_some();
	state.db.flush().expect("flush");
    if existed { Json(serde_json::json!({ "deleted": true, "cascaded": true })).into_response() } else { json_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Memory not found", None) }
}

async fn maintenance_loop(state: Arc<AppState>) {
	let interval_ms: u64 = std::env::var("STM_CLEAN_INTERVAL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(60_000);
	loop {
		if let Err(err) = run_maintenance(&state) { error!(%err, "maintenance error"); }
        prune_query_cache(&state).await;
		sleep(Duration::from_millis(interval_ms)).await;
	}
}

fn run_maintenance(state: &Arc<AppState>) -> Result<()> {
	let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
	let decay: f64 = std::env::var("LTM_DECAY_PER_CLEAN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.99);
	let tree = state.db.open_tree("memories")?;
	for kv in tree.iter() {
		let (k, v) = kv?;
		let mut rec: serde_json::Value = serde_json::from_slice(&v).unwrap_or(serde_json::json!({}));
		let layer = rec.get("layer").and_then(|c| c.as_str()).unwrap_or("");
		if layer == "STM" {
			if let Some(exp) = rec.get("expires_at").and_then(|c| c.as_i64()) { if exp <= now_ms { let _ = tree.remove(k); continue; } }
            // Promotion: if STM has high importance or frequently accessed, promote to LTM
            let importance = rec.get("importance").and_then(|c| c.as_f64()).unwrap_or(1.0);
            let accessed = rec.get("access_count").and_then(|c| c.as_u64()).unwrap_or(0);
            let promote_threshold = std::env::var("CONSOLIDATE_IMPORTANCE_MIN").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.5);
            let access_threshold = std::env::var("CONSOLIDATE_ACCESS_MIN").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(3);
            if importance >= promote_threshold || accessed >= access_threshold {
                rec["layer"] = serde_json::json!("LTM");
                rec["promoted_at"] = serde_json::json!(now_ms);
                tree.insert(&k, serde_json::to_vec(&rec)?)?;
            }
		} else if layer == "LTM" {
			let imp = rec.get("importance").and_then(|c| c.as_f64()).unwrap_or(1.0) * decay;
			rec["importance"] = serde_json::json!(imp);
			tree.insert(&k, serde_json::to_vec(&rec)?)?;
		}
		// Scheduled promotion based on thresholds
		let importance = rec.get("importance").and_then(|c| c.as_f64()).unwrap_or(1.0);
		let accessed = rec.get("access_count").and_then(|c| c.as_u64()).unwrap_or(0);
		let promote_threshold = std::env::var("CONSOLIDATE_IMPORTANCE_MIN").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.5);
		let access_threshold = std::env::var("CONSOLIDATE_ACCESS_MIN").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(3);
		if importance >= promote_threshold || accessed >= access_threshold {
			rec["layer"] = serde_json::json!("LTM");
			rec["promoted_at"] = serde_json::json!(now_ms);
			tree.insert(&k.clone(), serde_json::to_vec(&rec)?)?;
			// Audit (best-effort)
			if let Ok(log) = state.db.open_tree("consolidation_log") {
				let id = rec.get("id").and_then(|c| c.as_str()).unwrap_or("");
				let reason = if importance >= promote_threshold { "importance" } else { "access" };
				let log_key = format!("{}:{}", now_ms, id);
				let log_val = serde_json::json!({ "id": id, "from": "STM", "to": "LTM", "reason": reason, "ts": now_ms });
				let _ = log.insert(log_key.as_bytes(), serde_json::to_vec(&log_val)?);
			}
		}
	}
	// Enforce STM LRU capacity if configured
	let max_items: usize = std::env::var("STM_MAX_ITEMS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
	if max_items > 0 {
		let mut stm_items: Vec<(sled::IVec, i64)> = Vec::new();
		for kv in tree.iter() {
			let (k, v) = kv?;
			let rec: serde_json::Value = serde_json::from_slice(&v).unwrap_or(serde_json::json!({}));
			let layer = rec.get("layer").and_then(|c| c.as_str()).unwrap_or("");
			if layer == "STM" {
				let ts = rec.get("last_access_ts").and_then(|c| c.as_i64()).or_else(|| rec.get("created_at").and_then(|c| c.as_i64())).unwrap_or(now_ms);
				stm_items.push((k, ts));
			}
		}
		if stm_items.len() > max_items {
			stm_items.sort_by_key(|(_, ts)| *ts);
			let to_remove = stm_items.len() - max_items;
			for (k, _) in stm_items.into_iter().take(to_remove) {
				let _ = tree.remove(k);
			}
		}
	}
	state.db.flush()?;
	Ok(())
}

async fn prune_query_cache(state: &Arc<AppState>) {
    let ttl_ms: i64 = std::env::var("FUSION_CACHE_TTL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(3_000);
    let max_entries: usize = std::env::var("FUSION_CACHE_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(1_000);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let mut guard = state.query_cache.lock().await;
    // TTL prune
    let keys_to_remove: Vec<String> = guard.iter().filter_map(|(k,(ts,_))| if now - *ts > ttl_ms { Some(k.clone()) } else { None }).collect();
    for k in keys_to_remove { guard.remove(&k); }
    // Size prune (LRU-ish by oldest ts)
    if guard.len() > max_entries {
        let mut items: Vec<(String, i64)> = guard.iter().map(|(k,(ts,_))| (k.clone(), *ts)).collect();
        items.sort_by_key(|(_, ts)| *ts); // oldest first
        let to_remove = guard.len() - max_entries;
        for (k, _) in items.into_iter().take(to_remove) { guard.remove(&k); }
    }
}

async fn advanced_consolidate(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let limit = body.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let dry = body.get("dryRun").and_then(|v| v.as_bool()).unwrap_or(false);
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let promote_threshold = std::env::var("CONSOLIDATE_IMPORTANCE_MIN").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(1.5);
    let access_threshold = std::env::var("CONSOLIDATE_ACCESS_MIN").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(3);
    let tree = state.db.open_tree("memories").expect("mem tree");
    let mut promoted = 0usize;
    let mut candidates = 0usize;
    for kv in tree.iter() {
        if promoted >= limit { break; }
        let (k, v) = kv.expect("ok");
        let mut rec: serde_json::Value = serde_json::from_slice(&v).unwrap_or(serde_json::json!({}));
        let layer = rec.get("layer").and_then(|c| c.as_str()).unwrap_or("");
        if layer != "STM" { continue; }
        let importance = rec.get("importance").and_then(|c| c.as_f64()).unwrap_or(1.0);
        let accessed = rec.get("access_count").and_then(|c| c.as_u64()).unwrap_or(0);
        if importance >= promote_threshold || accessed >= access_threshold {
            candidates += 1;
            if !dry {
                rec["layer"] = serde_json::json!("LTM");
                rec["promoted_at"] = serde_json::json!(now_ms);
                tree.insert(k, serde_json::to_vec(&rec).expect("ser"))
                    .expect("insert");
                promoted += 1;
            }
        }
    }
    state.db.flush().expect("flush");
    Json(serde_json::json!({ "promoted": promoted, "candidates": candidates, "tookMs": 0 }))
}

async fn search_fusion(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Json<SearchResponse> {
    let started = std::time::Instant::now();
	let q = params.get("q").cloned().unwrap_or_default().to_lowercase();
	let limit = params.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
    let time_from = params.get("from").and_then(|s| s.parse::<i64>().ok());
    let time_to = params.get("to").and_then(|s| s.parse::<i64>().ok());
    let cache_key = format!("q={}::limit={}", q, limit);
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    // Serve from cache if fresh
    if let Some(cached) = {
        let guard = state.query_cache.lock().await;
        guard.get(&cache_key).map(|(ts, items)| (*ts, items.clone()))
    } {
        let (ts, mut items) = cached;
        if now_ms - ts <= std::env::var("FUSION_CACHE_TTL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(3_000) {
            items.truncate(limit);
            // metrics update: cache hit
            {
                let mut m = state.metrics.lock().await;
                m.count += 1; m.cache_hits += 1; m.last_ms = 0;
                // history window for percentiles/QPS
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
                m.history.push_back((now, 0));
                while let Some((ts,_)) = m.history.front().cloned() { if now - ts > 60_000 { m.history.pop_front(); } else { break; } }
                let mut lat: Vec<u64> = m.history.iter().map(|(_,v)| *v).collect();
                lat.sort_unstable();
                if !lat.is_empty() {
                    let p50_idx = (lat.len() as f64 * 0.5).floor() as usize;
                    let p95_idx = (lat.len() as f64 * 0.95).floor().min((lat.len()-1) as f64) as usize;
                    m.p50_ms = lat[p50_idx] as f64; m.p95_ms = lat[p95_idx] as f64;
                }
                m.qps_1m = m.history.len() as f64 / 60.0;
            }
            return Json(SearchResponse { results: items, took_ms: Some(0) });
        }
    }
	// Text: naive scan of tantivy is non-trivial; reuse memories substring for demo and include doc chunks via sled text_index fallback
	let mut results: Vec<SearchResult> = Vec::new();
    // From memories (apply temporal filters if provided)
	let tree = state.db.open_tree("memories").expect("mem");
	for kv in tree.iter() {
		let (_, v) = kv.expect("ok");
		if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
			let content = rec.get("content").and_then(|c| c.as_str()).unwrap_or("").to_lowercase();
            let created_at = rec.get("created_at").and_then(|c| c.as_i64());
            let in_time = created_at.map(|t| time_from.map(|f| t>=f).unwrap_or(true) && time_to.map(|to| t<=to).unwrap_or(true)).unwrap_or(true);
            if content.contains(&q) && in_time {
				let id = rec.get("id").and_then(|c| c.as_str()).unwrap_or("").to_string();
				let layer_v = rec.get("layer").and_then(|c| c.as_str()).unwrap_or("").to_string();
				let refs = rec.get("docRefs").and_then(|r| r.as_array()).map(|arr| {
					arr.iter().filter_map(|x| {
						let doc_id = x.get("docId").and_then(|v| v.as_str())?.to_string();
						let chunk_id = x.get("chunkId").and_then(|v| v.as_str()).map(|s| s.to_string());
						let score = x.get("score").and_then(|v| v.as_f64()).map(|f| f as f32);
						Some(DocRefOut{ doc_id, chunk_id, score })
					}).collect::<Vec<_>>()
				});
                results.push(SearchResult{ id, score: 0.0, layer: layer_v, doc_refs: refs, explain: Some(serde_json::json!({"text": 1.0})) });
			}
		}
	}
	// From doc text index (sled fallback)
	if let Ok(text_idx) = state.db.open_tree("text_index") {
        for kv in text_idx.iter() { if let Ok((k,v)) = kv { let s = String::from_utf8_lossy(&v).to_lowercase(); if s.contains(&q) { let id = String::from_utf8(k.to_vec()).unwrap_or_default(); results.push(SearchResult{ id, score: 0.0, layer: "doc".to_string(), doc_refs: None, explain: Some(serde_json::json!({"text": 1.0, "source":"doc-index"})) }); } } }
	}
	results.sort_by(|a,b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
	results.truncate(limit);
    // KG semantic hits: if query matches an entity, include memories that mention it
    if !q.is_empty() {
        if let Ok(edges) = state.db.open_tree("kg_edges") {
            let needle = format!("->Entity::{}::MENTIONS", q);
            for kv in edges.iter() {
                if let Ok((k, _)) = kv {
                    let key = String::from_utf8(k.to_vec()).unwrap_or_default();
                    if key.ends_with(&needle) || key.to_lowercase().ends_with(&needle.to_lowercase()) {
                        if let Some((src, _)) = key.split_once("->") {
                            if let Some(mem_id) = src.strip_prefix("Memory::") {
                                let already = results.iter().any(|r| r.id == mem_id);
                                if !already {
                                    let layer_v = if let Ok(Some(v)) = tree.get(mem_id.as_bytes()) { serde_json::from_slice::<serde_json::Value>(&v).ok().and_then(|r| r.get("layer").and_then(|x| x.as_str()).map(|s| s.to_string())).unwrap_or_else(|| "STM".to_string()) } else { "STM".to_string() };
                                    results.push(SearchResult { id: mem_id.to_string(), score: 0.0, layer: layer_v, doc_refs: None, explain: Some(serde_json::json!({"kg": 1.0})) });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Vector ANN augmentation via neighbor graph
    if !q.is_empty() {
        let qvec = embeddings::embed_batch(&[q.as_str()]);
        if let Some(vec) = qvec.get(0) {
            let topk = vector_index::ann_search_memories(&state.db, vec, limit);
            for (id, score) in topk {
                let already = results.iter().any(|r| r.id == id);
                if !already {
                    let layer_v = if let Ok(Some(v)) = tree.get(id.as_bytes()) { serde_json::from_slice::<serde_json::Value>(&v).ok().and_then(|r| r.get("layer").and_then(|x| x.as_str()).map(|s| s.to_string())).unwrap_or_else(|| "STM".to_string()) } else { "STM".to_string() };
                    results.push(SearchResult { id, score: 0.0, layer: layer_v, doc_refs: None, explain: Some(serde_json::json!({"vector": score, "source":"vector-ann"})) });
                }
            }
        }
    }
    // Cache after augmentation
    {
        let mut guard = state.query_cache.lock().await;
        guard.insert(cache_key, (now_ms, results.clone()));
    }
    let took = started.elapsed().as_millis() as u64;
    // metrics update: cache miss
    {
        let mut m = state.metrics.lock().await;
        m.count += 1; m.cache_misses += 1; m.last_ms = took; m.avg_ms = ((m.avg_ms * ((m.count.saturating_sub(1)) as f64)) + took as f64) / (m.count as f64);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        m.history.push_back((now, took));
        while let Some((ts,_)) = m.history.front().cloned() { if now - ts > 60_000 { m.history.pop_front(); } else { break; } }
        let mut lat: Vec<u64> = m.history.iter().map(|(_,v)| *v).collect();
        lat.sort_unstable();
        if !lat.is_empty() {
            let p50_idx = (lat.len() as f64 * 0.5).floor() as usize;
        let p95_idx = (lat.len() as f64 * 0.95).floor().min((lat.len()-1) as f64) as usize;
            m.p50_ms = lat[p50_idx] as f64; m.p95_ms = lat[p95_idx] as f64;
        }
        m.qps_1m = m.history.len() as f64 / 60.0;
    }
    Json(SearchResponse { results, took_ms: Some(took as u128) })
}

async fn document_refs_for_memory(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
    let mem_id = match params.get("id").cloned() { Some(s) => s, None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "id required", None) };
    let refs_tree = state.db.open_tree("doc_refs").expect("doc_refs");
    let prefix = format!("mem::{}::", mem_id);
    let mut out: Vec<serde_json::Value> = Vec::new();
    for kv in refs_tree.scan_prefix(prefix.as_bytes()) {
        if let Ok((k, v)) = kv {
            let key = String::from_utf8_lossy(&k);
            // mem::<id>::doc::<docId>::chunk::<chunkId>
            let parts: Vec<&str> = key.split("::").collect();
            if parts.len() >= 6 {
                let doc_id = parts[3].to_string();
                let chunk_id = if parts.len() >= 6 { Some(parts[5].to_string()) } else { None };
                let score = serde_json::from_slice::<serde_json::Value>(&v).ok().and_then(|x| x.get("score").and_then(|s| s.as_f64())).unwrap_or(0.0);
                out.push(serde_json::json!({ "docId": doc_id, "chunkId": chunk_id, "score": score }));
            }
        }
    }
    Json(serde_json::json!({ "id": mem_id, "docRefs": out })).into_response()
}

async fn document_refs_for_document(axum::extract::State(state): axum::extract::State<Arc<AppState>>, axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
    let doc_id = match params.get("id").cloned() { Some(s) => s, None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "id required", None) };
    let refs_tree = state.db.open_tree("doc_refs").expect("doc_refs");
    let needle = format!("::doc::{}::", doc_id);
    let mut out: Vec<serde_json::Value> = Vec::new();
    for kv in refs_tree.iter() {
        if let Ok((k, v)) = kv {
            let key = String::from_utf8_lossy(&k);
            if key.contains(&needle) {
                let parts: Vec<&str> = key.split("::").collect();
                if parts.len() >= 6 {
                    let mem_id = parts[1].to_string();
                    let chunk_id = if parts.len() >= 6 { Some(parts[5].to_string()) } else { None };
                    let score = serde_json::from_slice::<serde_json::Value>(&v).ok().and_then(|x| x.get("score").and_then(|s| s.as_f64())).unwrap_or(0.0);
                    out.push(serde_json::json!({ "memoryId": mem_id, "chunkId": chunk_id, "score": score }));
                }
            }
        }
    }
    Json(serde_json::json!({ "id": doc_id, "memories": out })).into_response()
}

#[derive(Deserialize)]
struct ValidateRefsBody { fix: Option<bool> }

async fn document_validate_refs(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<ValidateRefsBody>) -> Json<serde_json::Value> {
    let fix = body.fix.unwrap_or(false);
    let refs_tree = state.db.open_tree("doc_refs").expect("doc_refs");
    let mems = state.db.open_tree("memories").expect("memories");
    let docs_meta = state.db.open_tree("chunks").expect("chunks");
    let mut invalid: Vec<String> = Vec::new();
    let mut removed = 0u64;
    for kv in refs_tree.iter() {
        if let Ok((k, _)) = kv {
            let key = String::from_utf8_lossy(&k);
            // mem::<id>::doc::<docId>::chunk::<chunkId>
            let parts: Vec<&str> = key.split("::").collect();
            if parts.len() < 6 { invalid.push(key.to_string()); if fix { let _ = refs_tree.remove(&k); removed += 1; } continue; }
            let mem_id = parts[1];
            let doc_id = parts[3];
            let chunk_id = parts[5];
            let mem_ok = mems.get(mem_id.as_bytes()).ok().flatten().is_some();
            // minimal doc/chunk check: presence of any chunk for doc
            let prefix = format!("{}:", doc_id);
            let mut doc_ok = false;
            for it in docs_meta.scan_prefix(prefix.as_bytes()).take(1) { if it.is_ok() { doc_ok = true; break; } }
            if !mem_ok || !doc_ok || chunk_id.is_empty() {
                invalid.push(key.to_string());
                if fix { let _ = refs_tree.remove(&k); removed += 1; }
            }
        }
    }
    Json(serde_json::json!({ "invalid": invalid, "removed": if fix { Some(removed) } else { None } }))
}

fn index_chunks_sled(db: &sled::Db, doc_id: &str, chunks: &[ChunkHeader], full_text: &str) -> Result<()> {
	let text_idx = db.open_tree("text_index")?;
	for ch in chunks {
		let start = ch.position.start;
		let end = ch.position.end.min(full_text.len());
		let text_slice = &full_text[start..end];
		let key = format!("{}:{}", doc_id, start);
		text_idx.insert(key.as_bytes(), text_slice.as_bytes())?;
	}
	Ok(())
}

fn run_index_maintenance(state: &Arc<AppState>) -> Result<(u64, u64)> {
	let text_idx = state.db.open_tree("text_index")?;
	let chunks = state.db.open_tree("chunks")?;
	let mut removed_text = 0u64;
	for kv in text_idx.iter() {
		let (k, _) = kv?;
		let key = String::from_utf8(k.to_vec()).unwrap_or_default();
		if let Some((doc_id, _)) = key.split_once(":") {
			let prefix = format!("{}:", doc_id);
			let mut has_chunks = false;
			for it in chunks.scan_prefix(prefix.as_bytes()).take(1) { if it.is_ok() { has_chunks = true; break; } }
			if !has_chunks { let _ = text_idx.remove(k); removed_text += 1; }
		}
	}
	let nodes = state.db.open_tree("kg_nodes")?;
	let edges = state.db.open_tree("kg_edges")?;
	let mut removed_edges = 0u64;
	for kv in edges.iter() {
		let (k, v) = kv?;
		let val: serde_json::Value = serde_json::from_slice(&v).unwrap_or(serde_json::json!({}));
		let src = val.get("src").and_then(|c| c.as_str()).unwrap_or("");
		let dst = val.get("dst").and_then(|c| c.as_str()).unwrap_or("");
		let src_exists = nodes.get(src.as_bytes())?.is_some();
		let dst_exists = nodes.get(dst.as_bytes())?.is_some();
		if !src_exists || !dst_exists { let _ = edges.remove(k); removed_edges += 1; }
	}
    // Clean orphan memory embeddings
    let removed_emb = vector_index::cleanup_orphan_mem_embeddings(&state.db).unwrap_or(0);
    state.db.flush()?;
    Ok((removed_text + removed_emb, removed_edges))
}

async fn system_cleanup(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
	let reindex = body.get("reindex").and_then(|v| v.as_bool()).unwrap_or(false);
	let compact = body.get("compact").and_then(|v| v.as_bool()).unwrap_or(false);
	let (removed_text, removed_edges) = run_index_maintenance(&state).unwrap_or((0,0));
	if compact { let _ = state.db.flush(); }
	Json(serde_json::json!({ "removedText": removed_text, "removedEdges": removed_edges, "reindexed": reindex, "compacted": compact }))
}

async fn system_validate(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Basic integrity checks: embeddings dimension, orphan embeddings, KG edge endpoints
    let (total, invalid) = vector_index::validate_mem_embeddings(&state.db);
    let mut orphan = 0u64;
    if let Ok(tree) = state.db.open_tree("mem_embeddings") {
        if let Ok(mems) = state.db.open_tree("memories") { for kv in tree.iter() { if let Ok((k,_)) = kv { if mems.get(&k).ok().flatten().is_none() { orphan += 1; } } } }
    }
    let mut bad_edges = 0u64;
    if let (Ok(nodes), Ok(edges)) = (state.db.open_tree("kg_nodes"), state.db.open_tree("kg_edges")) {
        for kv in edges.iter() {
            if let Ok((_, v)) = kv {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&v) {
                    let src = val.get("src").and_then(|c| c.as_str()).unwrap_or("");
                    let dst = val.get("dst").and_then(|c| c.as_str()).unwrap_or("");
                    if nodes.get(src.as_bytes()).ok().flatten().is_none() || nodes.get(dst.as_bytes()).ok().flatten().is_none() { bad_edges += 1; }
                }
            }
        }
    }
    Json(serde_json::json!({ "embeddings": { "total": total, "invalid": invalid, "orphans": orphan }, "kg": { "badEdges": bad_edges } }))
}

async fn system_backup(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
    let dest = body.get("destination").and_then(|v| v.as_str()).map(|s| s.to_string())
        .or_else(|| std::env::var("BACKUP_DIR").ok())
        .unwrap_or_else(|| "./backup".to_string());
    let include_indices = body.get("includeIndices").and_then(|v| v.as_bool()).unwrap_or(true);
    match create_backup(&state, &dest, include_indices) {
        Ok((path, size_mb, took_ms)) => Json(serde_json::json!({ "path": path, "sizeMb": size_mb, "tookMs": took_ms })).into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
    }
}

async fn system_restore(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Response {
    let source = match body.get("source").and_then(|v| v.as_str()) { Some(s) => s.to_string(), None => return json_error(StatusCode::BAD_REQUEST, "INVALID_INPUT", "source required", None) };
    let include_indices = body.get("includeIndices").and_then(|v| v.as_bool()).unwrap_or(true);
    match restore_backup(&state, &source, include_indices) {
        Ok(took_ms) => {
            // Validate manifest exists
            let man = std::path::Path::new(&source).join("manifest.json");
            let valid = man.exists();
            Json(serde_json::json!({ "restored": true, "validated": valid, "tookMs": took_ms })).into_response()
        },
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
    }
}

async fn system_compact(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(_body): Json<serde_json::Value>) -> Response {
    // Best-effort compaction: flush sled, rebuild vector neighbor graph, and tantivy merge by reindex
    let _ = state.db.flush();
    let _ = vector_index::build_mem_neighbor_graph(&state.db, 16);
    // Tantivy merge: trigger a lightweight reindex of memory docs
    if let Ok(tree) = state.db.open_tree("memories") {
        for kv in tree.iter() {
            if let Ok((_, v)) = kv {
                if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
                    if let Some(id) = rec.get("id").and_then(|x| x.as_str()) {
                        let content = rec.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        let _ = index_memory_tantivy(&state.index_dir, id, content);
                    }
                }
            }
        }
    }
    Json(serde_json::json!({ "compacted": true })).into_response()
}

#[derive(Deserialize)]
struct ExportBody { #[serde(default)] include_indices: Option<bool> }

async fn data_export(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<ExportBody>) -> Response {
    let dest = std::env::var("EXPORT_DIR").unwrap_or_else(|_| "./export".to_string());
    let include_indices = body.include_indices.unwrap_or(true);
    match create_backup(&state, &dest, include_indices) {
        Ok((path, size_mb, took_ms)) => Json(serde_json::json!({ "path": path, "sizeMb": size_mb, "tookMs": took_ms })).into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
    }
}

#[derive(Deserialize)]
struct ImportBody { source: String, #[serde(default)] include_indices: Option<bool> }

async fn data_import(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<ImportBody>) -> Response {
    let include_indices = body.include_indices.unwrap_or(true);
    match restore_backup(&state, &body.source, include_indices) {
        Ok(took_ms) => Json(serde_json::json!({ "imported": true, "tookMs": took_ms })).into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", err.to_string(), None)
    }
}

fn dir_size_mb(path: &std::path::Path) -> u64 {
    fn walk(p: &std::path::Path) -> u64 {
        let mut total = 0u64;
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                let m = match e.metadata() { Ok(m) => m, Err(_) => continue };
                if m.is_dir() { total += walk(&e.path()); } else { total += m.len(); }
            }
        }
        total
    }
    (walk(path) / (1024*1024)) as u64
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let to = dst.join(entry.file_name());
        if path.is_dir() { copy_dir(&path, &to)?; } else { let _ = std::fs::copy(&path, &to); }
    }
    Ok(())
}

fn create_backup(_state: &Arc<AppState>, destination: &str, include_indices: bool) -> Result<(String, u64, u128)> {
    use std::time::Instant as TInstant;
    let started = TInstant::now();
    let data_root = std::path::PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()));
    let dest = std::path::PathBuf::from(destination);
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let target = dest.join(format!("snapshot-{}", ts));
    std::fs::create_dir_all(&target)?;
    // Warm and cold tiers
    let warm = data_root.join("warm");
    let cold = data_root.join("cold");
    if warm.exists() { copy_dir(&warm, &target.join("warm"))?; }
    if cold.exists() { copy_dir(&cold, &target.join("cold"))?; }
    if include_indices {
        let index = data_root.join("index");
        if index.exists() { copy_dir(&index, &target.join("index"))?; }
    }
    let size_mb = dir_size_mb(&target);
    let took = started.elapsed().as_millis();
    // Write manifest
    let manifest = serde_json::json!({
        "createdAt": ts,
        "includeIndices": include_indices,
        "sizesMb": { "warmColdIndex": size_mb }
    });
    let _ = std::fs::write(target.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?);
    Ok((target.to_string_lossy().to_string(), size_mb, took))
}

fn restore_backup(_state: &Arc<AppState>, source: &str, include_indices: bool) -> Result<u128> {
    use std::time::Instant as TInstant;
    let started = TInstant::now();
    let src = std::path::PathBuf::from(source);
    let data_root = std::path::PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()));
    // Restore into staging, then atomically move directories where safe.
    let warm_src = src.join("warm");
    let cold_src = src.join("cold");
    let index_src = src.join("index");
    if warm_src.exists() { copy_dir(&warm_src, &data_root.join("warm"))?; }
    if cold_src.exists() { copy_dir(&cold_src, &data_root.join("cold"))?; }
    if include_indices && index_src.exists() { copy_dir(&index_src, &data_root.join("index"))?; }
    Ok(started.elapsed().as_millis())
}

async fn advanced_reindex(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let vector = body.get("vector").and_then(|v| v.as_bool()).unwrap_or(true);
    let text = body.get("text").and_then(|v| v.as_bool()).unwrap_or(true);
    let graph = body.get("graph").and_then(|v| v.as_bool()).unwrap_or(true);
    // Placeholder: run maintenance to prune; reindex text by reinserting current content
    let _ = run_index_maintenance(&state);
    if text {
        if let Ok(tree) = state.db.open_tree("memories") {
            for kv in tree.iter() {
                if let Ok((_, v)) = kv {
                    if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
                        if let Some(id) = rec.get("id").and_then(|x| x.as_str()) {
                            let content = rec.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            let _ = index_memory_sled(&state.db, id, content);
                            let _ = index_memory_tantivy(&state.index_dir, id, content);
                        }
                    }
                }
            }
        }
    }
    if vector {
        let _ = vector_index::reembed_all_memories(&state.db, 256);
        let _ = vector_index::build_mem_neighbor_graph(&state.db, 16);
    }
    Json(serde_json::json!({ "vector": vector, "text": text, "graph": graph, "tookMs": 0 }))
}

async fn advanced_analyze_patterns(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let from = body.get("window").and_then(|w| w.get("from")).and_then(|v| v.as_i64());
    let to = body.get("window").and_then(|w| w.get("to")).and_then(|v| v.as_i64());
    let min_support = body.get("minSupport").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    let tree = state.db.open_tree("memories").expect("mem");
    let mut counter: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for kv in tree.iter() {
        if let Ok((_, v)) = kv {
            if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
                let created_at = rec.get("created_at").and_then(|c| c.as_i64());
                let in_time = created_at.map(|t| from.map(|f| t>=f).unwrap_or(true) && to.map(|to| t<=to).unwrap_or(true)).unwrap_or(true);
                if !in_time { continue; }
                if let Some(content) = rec.get("content").and_then(|c| c.as_str()) {
                    for ent in kg::extract_entities(content) { *counter.entry(ent).or_insert(0) += 1; }
                }
            }
        }
    }
    let mut patterns: Vec<(String, usize)> = counter.into_iter().filter(|(_, c)| *c >= min_support).collect();
    patterns.sort_by(|a, b| b.1.cmp(&a.1));
    let out: Vec<serde_json::Value> = patterns.into_iter().map(|(concept, support)| serde_json::json!({ "concept": concept, "support": support, "trend": "flat" })).collect();
    Json(serde_json::json!({ "patterns": out }))
}

async fn advanced_trends(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let from = body.get("from").and_then(|v| v.as_i64());
    let to = body.get("to").and_then(|v| v.as_i64());
    let buckets = body.get("buckets").and_then(|v| v.as_u64()).unwrap_or(10) as i64;
    let tree = state.db.open_tree("memories").expect("mem");
    let mut timeline: Vec<serde_json::Value> = Vec::new();
    if let (Some(f), Some(t)) = (from, to) {
        let span = (t - f).max(1);
        let step = (span / buckets).max(1);
        for i in 0..buckets {
            let start = f + i * step;
            let end = if i == buckets-1 { t } else { start + step - 1 };
            let mut stm = 0u64; let mut ltm = 0u64;
            for kv in tree.iter() {
                if let Ok((_, v)) = kv {
                    if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
                        if let (Some(ts), Some(layer)) = (rec.get("created_at").and_then(|x| x.as_i64()), rec.get("layer").and_then(|x| x.as_str())) {
                            if ts >= start && ts <= end { if layer == "STM" { stm += 1; } else if layer == "LTM" { ltm += 1; } }
                        }
                    }
                }
            }
            timeline.push(serde_json::json!({ "start": start, "end": end, "STM": stm, "LTM": ltm }));
        }
    }
    Json(serde_json::json!({ "timeline": timeline }))
}

async fn advanced_clusters(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(_body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    // Simple clustering: documents linked by RELATED edges -> connected components
    let edges = state.db.open_tree("kg_edges").expect("edges");
    let mut graph: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for kv in edges.iter() {
        if let Ok((k, _)) = kv {
            let key = String::from_utf8_lossy(&k);
            if key.ends_with("::RELATED") {
                if let Some((src, rest)) = key.split_once("->") {
                    let dst = rest.split("::").next().unwrap_or("");
                    graph.entry(src.to_string()).or_default().push(dst.to_string());
                    graph.entry(dst.to_string()).or_default().push(src.to_string());
                }
            }
        }
    }
    // Connected components
    let mut seen = std::collections::HashSet::new();
    let mut clusters: Vec<Vec<String>> = Vec::new();
    for node in graph.keys() {
        if seen.contains(node) { continue; }
        let mut stack = vec![node.clone()];
        let mut comp: Vec<String> = Vec::new();
        while let Some(n) = stack.pop() {
            if !seen.insert(n.clone()) { continue; }
            comp.push(n.clone());
            if let Some(nei) = graph.get(&n) { for m in nei { if !seen.contains(m) { stack.push(m.clone()); } } }
        }
        if comp.len() > 1 { clusters.push(comp); }
    }
    // Normalize to doc ids
    let out: Vec<serde_json::Value> = clusters.into_iter().map(|c| {
        let docs: Vec<String> = c.into_iter().filter_map(|n| n.strip_prefix("Document::").map(|s| s.to_string())).collect();
        serde_json::json!({ "docs": docs })
    }).collect();
    Json(serde_json::json!({ "clusters": out }))
}

async fn advanced_relationships(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(_body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    // Relationship strength: count edges per (src_type, relation, dst_type)
    let edges = state.db.open_tree("kg_edges").expect("edges");
    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for kv in edges.iter() {
        if let Ok((k, _)) = kv {
            let key = String::from_utf8_lossy(&k);
            if let Some((src, rest)) = key.split_once("->") {
                if let Some((dst, rel)) = rest.split_once("::") {
                    let src_t = src.split("::").next().unwrap_or("");
                    let dst_t = dst.split("::").next().unwrap_or("");
                    let grp = format!("{}:{}:{}", src_t, rel, dst_t);
                    *counts.entry(grp).or_insert(0) += 1;
                }
            }
        }
    }
    let mut items: Vec<(String, u64)> = counts.into_iter().collect();
    items.sort_by(|a,b| b.1.cmp(&a.1));
    let out: Vec<serde_json::Value> = items.into_iter().map(|(k, v)| serde_json::json!({ "group": k, "count": v })).collect();
    Json(serde_json::json!({ "relationships": out }))
}

async fn advanced_effectiveness(axum::extract::State(state): axum::extract::State<Arc<AppState>>, Json(_body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    // Effectiveness heuristic: combine access_count, importance, recency into a score
    let mems = state.db.open_tree("memories").expect("mem");
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let half_life_ms: f64 = std::env::var("EFFECT_HALF_LIFE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(30.0*24.0*3600.0*1000.0);
    let mut out: Vec<serde_json::Value> = Vec::new();
    for kv in mems.iter() {
        if let Ok((_, v)) = kv {
            if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) {
                let id = rec.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let imp = rec.get("importance").and_then(|x| x.as_f64()).unwrap_or(1.0);
                let acc = rec.get("access_count").and_then(|x| x.as_u64()).unwrap_or(0) as f64;
                let ts = rec.get("created_at").and_then(|x| x.as_i64()).unwrap_or(now_ms);
                let age = (now_ms - ts).max(0) as f64;
                let recency = (-(age/half_life_ms)).exp();
                let score = imp * (1.0 + acc.log10().max(0.0)) * recency;
                out.push(serde_json::json!({ "id": id, "score": score }));
            }
        }
    }
    out.sort_by(|a,b| b.get("score").and_then(|x| x.as_f64()).partial_cmp(&a.get("score").and_then(|x| x.as_f64())).unwrap_or(std::cmp::Ordering::Equal));
    Json(serde_json::json!({ "effectiveness": out }))
}

async fn shutdown_signal() {
	let _ = signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State as AxState;
    use axum::Json;
    use std::sync::Arc;
    use std::collections::HashMap as Map;
    use rand::{Rng, distributions::Alphanumeric};

    fn make_state() -> Arc<AppState> {
        let base = std::env::temp_dir().join(format!("mcp-test-{}", uuid::Uuid::new_v4()));
        let base_str = base.to_string_lossy().to_string();
        std::fs::create_dir_all(&base).unwrap();
        std::env::set_var("DATA_DIR", &base_str);
        let dirs = ensure_data_dirs(&base_str).unwrap();
        let db_path = dirs.warm.join("kv");
        let db = sled::open(db_path).unwrap();
        Arc::new(AppState {
            start_time: Instant::now(),
            db,
            index_dir: dirs.index,
            query_cache: AsyncMutex::new(HashMap::new()),
            metrics: AsyncMutex::new(QueryMetrics::default()),
            ingest_sema: Arc::new(Semaphore::new(4)),
            buf_pool: StdMutex::new(ByteBufPool::default()),
        })
    }

    #[tokio::test]
    async fn test_document_store_and_retrieve_by_path() {
        let state = make_state();
        let req = StoreDocRequest { path: Some("docs/doc1.md".to_string()), mime: Some("md".to_string()), content: Some("# Title\nHello world".to_string()), metadata: None };
        let resp = document_store(AxState(state.clone()), Json(req)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        // Verify latest id by path exists
        let latest = state.db.open_tree("doc_path_latest").unwrap();
        let id = String::from_utf8(latest.get("docs/doc1.md").unwrap().unwrap().to_vec()).unwrap();
        // Verify chunks present
        let chunks = state.db.open_tree("chunks").unwrap();
        let prefix = format!("{}:", id);
        let mut count = 0usize; for kv in chunks.scan_prefix(prefix.as_bytes()) { if kv.is_ok() { count += 1; } }
        assert!(count >= 1);
    }

    #[tokio::test]
    async fn test_memory_add_search_and_delete() {
        let state = make_state();
        // Add memory
        let add = AddMemoryRequest { content: "alpha bravo charlie".to_string(), metadata: None, layer_hint: None, session_id: None, episode_id: None, references: None };
        let resp = memory_add(AxState(state.clone()), Json(add)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        // Find id by scanning memories
        let mems = state.db.open_tree("memories").unwrap();
        let mut found_id = String::new();
        for kv in mems.iter() { if let Ok((_, v)) = kv { if let Ok(rec) = serde_json::from_slice::<serde_json::Value>(&v) { if rec.get("content").and_then(|c| c.as_str()) == Some("alpha bravo charlie") { found_id = rec.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(); break; } } } }
        assert!(!found_id.is_empty());
        // Search
        let mut q = Map::new(); q.insert("q".to_string(), "bravo".to_string());
        let out = memory_search(AxState(state.clone()), axum::extract::Query(q)).await;
        assert!(out.results.iter().any(|r| r.id == found_id));
        // Delete
        let del = DeleteMemoryRequest { id: found_id.clone(), backup: Some(false) };
        let del_resp = memory_delete(AxState(state.clone()), Json(del)).await;
        assert_eq!(del_resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_input_validation_errors() {
        let state = make_state();
        let bad = AddMemoryRequest { content: "".to_string(), metadata: None, layer_hint: None, session_id: None, episode_id: None, references: None };
        let resp = memory_add(AxState(state.clone()), Json(bad)).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let req = StoreDocRequest { path: None, mime: Some("md".to_string()), content: None, metadata: None };
        let resp2 = document_store(AxState(state.clone()), Json(req)).await;
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_export_import_and_validate() {
        let state = make_state();
        // Create one memory to persist
        let add = AddMemoryRequest { content: "persist me".to_string(), metadata: None, layer_hint: Some("STM".to_string()), session_id: None, episode_id: None, references: None };
        let _ = memory_add(AxState(state.clone()), Json(add)).await;
        // Export
        let dest = std::env::temp_dir().join(format!("mcp-backups-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dest).unwrap();
        let body = serde_json::json!({ "destination": dest.to_string_lossy(), "includeIndices": true });
        let resp = system_backup(AxState(state.clone()), Json(body)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        // Verify manifest exists in latest snapshot
        let mut latest: Option<std::path::PathBuf> = None;
        for entry in std::fs::read_dir(&dest).unwrap() { let p = entry.unwrap().path(); if p.is_dir() { latest = Some(p); } }
        let snap = latest.expect("snapshot");
        assert!(snap.join("manifest.json").exists());
        // Validate integrity endpoint
        let report = system_validate(AxState(state.clone())).await;
        let emb_obj = report.get("embeddings").unwrap();
        assert!(emb_obj.get("total").unwrap().as_u64().unwrap() >= 1);
        // Restore (no-op into same DATA_DIR)
        let body2 = serde_json::json!({ "source": snap.to_string_lossy(), "includeIndices": true });
        let resp2 = system_restore(AxState(state.clone()), Json(body2)).await;
        assert_eq!(resp2.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_load_concurrent_memory_add() {
        let state = make_state();
        let mut tasks = Vec::new();
        for i in 0..20 { let s = state.clone(); tasks.push(tokio::spawn(async move {
            let content = format!("common token {}", i);
            let add = AddMemoryRequest { content, metadata: None, layer_hint: None, session_id: None, episode_id: None, references: None };
            let _ = memory_add(AxState(s), Json(add)).await;
        })); }
        for t in tasks { let _ = t.await; }
        let mut q = Map::new(); q.insert("q".to_string(), "common".to_string());
        let out = memory_search(AxState(state.clone()), axum::extract::Query(q)).await;
        assert!(out.results.len() >= 10);
    }

    #[tokio::test]
    async fn test_fuzz_input_validation() {
        let state = make_state();
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let len: usize = rng.gen_range(0..2048);
            let s: String = (0..len).map(|_| rng.sample(Alphanumeric) as char).collect();
            let add = AddMemoryRequest { content: s, metadata: None, layer_hint: None, session_id: None, episode_id: None, references: None };
            let resp = memory_add(AxState(state.clone()), Json(add)).await;
            // Empty content should be rejected; non-empty should be OK
            if len == 0 { assert_eq!(resp.status(), StatusCode::BAD_REQUEST); }
            else { assert_eq!(resp.status(), StatusCode::OK); }
        }
    }
}
