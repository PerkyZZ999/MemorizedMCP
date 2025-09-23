# Testing Results

| Test | OK | Status | Snippet |
|---|---:|---:|---|
| GET /health | true | 200 | {"status":"ok"} |
| GET /status | true | 200 | {"uptime_ms":75408,"indices":{"vector":{"items":0},"text":{"docs":0},"graph":{"nodes":0,"edges":0}},"storage":{"hot_mb":0,"warm_mb":0,"cold_mb":0},"metrics":{"count":0,"cacheHits":0,"cacheMisses":0,"a |
| GET /metrics | true | 200 | # TYPE mcp_queries_total counter mcp_queries_total 0 # TYPE mcp_cache_hits_total counter mcp_cache_hits_total 0 # TYPE mcp_cache_misses_total counter mcp_cache_misses_total 0 # TYPE mcp_query_last_ms  |
| GET /tools | true | 200 | [{"name":"memory.add","description":"Add a memory entry"},{"name":"memory.search","description":"Hybrid search across indices"},{"name":"memory.update","description":"Update a memory entry"},{"name":" |
| POST /document/store md | true | 200 | {"id":"ea26366e-9943-42ac-8320-f9e5308d20d1","hash":"eb9c74a9c67073edc2230429baed861e4ed4114cdd99a290db5f1fd764996295","chunks":1} |
| GET /document/retrieve?id | true | 200 | {"chunks":[{"id":"8f926056-d119-4c42-b1b7-007865ec0f01","position":{"end":20,"start":0}}],"id":"ea26366e-9943-42ac-8320-f9e5308d20d1","metadata":null} |
| GET /document/analyze | true | 200 | {"docRefs":[],"entities":["Title"],"id":"ea26366e-9943-42ac-8320-f9e5308d20d1","keyConcepts":["Title"],"summary":"# Title`nHello world"} |
| POST /document/store path | true | 200 | {"id":"9680c544-f1f0-407d-a87b-0a8a13814dfc","hash":"9b7512c0fab6aea343b70dd461a71942de673849f8c8db125d35f8213221c413","chunks":1} |
| GET /document/retrieve?path | true | 200 | {"chunks":[{"id":"d0e9fff9-4a89-4943-8849-a44f2cf40ddf","position":{"end":36,"start":0}}],"id":"9680c544-f1f0-407d-a87b-0a8a13814dfc","metadata":null} |
| POST /document/validate_refs | true | 200 | {"invalid":[],"removed":null} |
| POST /memory/add | true | 200 | {"id":"b1e1262f-2dad-4d37-a06d-a30af889baa7","layer":"STM"} |
| GET /memory/search | true | 200 | {"results":[{"id":"b1e1262f-2dad-4d37-a06d-a30af889baa7","score":1.0,"layer":"STM"}],"tookMs":0} |
| POST /memory/add with refs | true | 200 | {"id":"8b3e27d6-bae0-46b1-a5ee-de5f20b5e445","layer":"STM"} |
| GET /document/refs_for_memory | true | 200 | {"docRefs":[{"chunkId":"","docId":"ea26366e-9943-42ac-8320-f9e5308d20d1","score":0.800000011920929}],"id":"8b3e27d6-bae0-46b1-a5ee-de5f20b5e445"} |
| GET /document/refs_for_document | true | 200 | {"id":"ea26366e-9943-42ac-8320-f9e5308d20d1","memories":[{"chunkId":"","memoryId":"8b3e27d6-bae0-46b1-a5ee-de5f20b5e445","score":0.800000011920929}]} |
| POST /memory/update | true | 200 | {"id":"b1e1262f-2dad-4d37-a06d-a30af889baa7","reembedded":true,"updatedIndices":["text","vector"],"version":1} |
| POST /memory/delete | true | 200 | {"cascaded":true,"deleted":true} |
| GET /search/fusion | true | 200 | {"results":[{"id":"8b3e27d6-bae0-46b1-a5ee-de5f20b5e445","score":0.0,"layer":"STM","explain":{"source":"vector-ann","vector":0.0}}],"tookMs":0} |
| POST /advanced/analyze_patterns | true | 200 | {"patterns":[]} |
| POST /advanced/trends | true | 200 | {"timeline":[{"LTM":0,"STM":0,"end":1758652910355,"start":1758652010356},{"LTM":0,"STM":0,"end":1758653810355,"start":1758652910356},{"LTM":0,"STM":0,"end":1758654710355,"start":1758653810356},{"LTM": |
| POST /advanced/clusters | true | 200 | {"clusters":[]} |
| POST /advanced/relationships | true | 200 | {"relationships":[{"count":1,"group":"TmpDoc:MENTIONS:9680c544-f1f0-407d-a87b-0a8a13814dfc"},{"count":1,"group":"This:MENTIONS:9680c544-f1f0-407d-a87b-0a8a13814dfc"},{"count":1,"group":"Memory:ea26366 |
| POST /advanced/effectiveness | true | 200 | {"effectiveness":[{"id":"8b3e27d6-bae0-46b1-a5ee-de5f20b5e445","score":0.9999998722993909}]} |
| POST /advanced/consolidate | true | 200 | {"candidates":0,"promoted":0,"tookMs":0} |
| POST /system/cleanup | true | 200 | {"compacted":true,"reindexed":false,"removedEdges":3,"removedText":1} |
| POST /advanced/reindex | true | 200 | {"graph":true,"text":true,"tookMs":0,"vector":true} |
| POST /system/backup | true | 200 | {"path":"./backups\\snapshot-1758655610565","sizeMb":0,"tookMs":49} |
| GET /system/validate | true | 200 | {"embeddings":{"invalid":0,"orphans":0,"total":1},"kg":{"badEdges":0}} |
| POST /system/restore | true | 200 | {"restored":true,"tookMs":34,"validated":true} |
| POST /document/store INVALID_INPUT | true | 400 | The remote server returned an error: (400) Bad Request. |
| POST /memory/add INVALID_INPUT | true | 400 | The remote server returned an error: (400) Bad Request. |
| GET /document/retrieve NOT_FOUND | true | 404 | The remote server returned an error: (404) Not Found. |

