# Neo4j Vector Search Implementation for Cascade KB

This document describes how vector search has been implemented in the Cascade Knowledge Base (KB) using Neo4j Community Edition.

## Overview

The cascade-kb crate now has full support for vector search capabilities using Neo4j Community Edition 5.18.0, which has native vector search support. This implementation allows for efficient semantic search of documentation chunks and other vector-embedded data.

## Requirements

- Neo4j Community Edition 5.18.0 or later
- Vector index support (included in Neo4j 5.18.0+)

## Implementation Details

### 1. Neo4j Configuration

The Neo4j database is configured with the following settings to enable vector search:

```yaml
environment:
  - NEO4J_dbms_security_procedures_unrestricted=apoc.*,db.index.vector.*
  - NEO4J_dbms_security_procedures_allowlist=apoc.*,db.index.vector.*
```

### 2. Vector Index Creation

Vector indexes are created using the Neo4j vector index API:

```cypher
CALL db.index.vector.createNodeIndex(
  'docEmbeddings', 
  'DocumentationChunk', 
  'embedding', 
  1536,  # dimensions
  'cosine'  # similarity metric
)
```

### 3. Data Model

The vector embeddings are stored in the following format:

```cypher
CREATE (d:DocumentationChunk {
  id: 'unique-uuid',
  tenant_id: 'tenant-id',
  text: 'Document content',
  embedding: [0.1, 0.2, 0.3, ...] // Vector embedding
})
```

### 4. Vector Search API

The `Neo4jStateStore` implementation provides a `vector_search` method with three search strategies:

1. **Index API**: Uses Neo4j's built-in vector index API for efficient vector search
   ```cypher
   CALL db.index.vector.queryNodes('docEmbeddings', k, embedding)
   YIELD node, score
   ```

2. **Vector Operator**: Fallback using Neo4j's vector similarity operator
   ```cypher
   MATCH (d:DocumentationChunk)
   WITH d, d.embedding <-> $embedding AS score
   ORDER BY score
   ```

3. **Fallback Search**: Simple node lookup when vector search isn't available

The implementation automatically tries each strategy in order and falls back to the next if one fails.

## Testing

The implementation has been tested with:

1. A basic docker-based test script that verifies vector index creation and search
2. Integration tests that validate the Neo4jStateStore's vector search functionality

## Usage Example

```rust
// Create a Neo4jStateStore
let store = Neo4jStateStore::new(config).await?;

// Create tenant ID and trace context
let tenant_id = TenantId::new_v4();
let trace_ctx = TraceContext::new_root();

// Perform vector search
let embedding = vec![0.2, 0.7, 0.8, 0.4, 0.6]; // Your embedding vector
let results = store.vector_search(
    &tenant_id,
    &trace_ctx,
    "docEmbeddings",
    &embedding,
    10, // top k results
    None // additional filters
).await?;

// Process results
for (id, score) in results {
    println!("Document ID: {}, Score: {}", id, score);
}
```

## Limitations

- The vector operator (`<->`) is not available in all Neo4j versions, but the index API is more widely supported
- Vector dimensions must match between the index definition and search vectors 