# Cascade Content Store Abstraction & Implementations

Provides the interface and concrete implementations for content-addressed storage (CAS) used by the Cascade Platform.

## Purpose

*   Define the `ContentStorage` trait for storing/retrieving immutable blobs via content hash.
*   Define traits/structs for managing `Manifest` documents.
*   Provide garbage collection functionality for managing storage.
*   Provide concrete implementations:
    *   `InMemoryKvStore`: For testing and local development.
    *   `RedisKvStore`: Using Redis as a backend.
    *   `CloudflareKvStore`: Using Cloudflare KV bindings (intended for `cascade-edge-manager` or WASM components running on CF).
    *   *(Potentially others: S3, DynamoDB, etc.)*

## Usage

Used by `cascade-server` (via `cascade-edge-manager`) to store component artifacts and manifests. Used by Edge Workers (via CF bindings) to retrieve manifests and components.

## Features

### Content-Addressed Storage

* Store and retrieve content using SHA-256 content hashes
* Automatic deduplication - identical content is stored only once
* Immutable content addressing ensures data integrity

### Manifest Management

* Store, retrieve, and delete flow manifests
* Track manifest access times for garbage collection purposes
* List all available manifests

### Garbage Collection

Garbage collection helps manage storage by:

* Removing orphaned content (content not referenced by any manifest)
* Removing inactive manifests (not accessed for a specified time)
* Providing metrics about the garbage collection process

#### Garbage Collection Options

```rust
pub struct GarbageCollectionOptions {
    // Age threshold for inactive manifests (ms)
    pub inactive_manifest_threshold_ms: u64,
    // Dry run mode - report but don't delete
    pub dry_run: bool,
    // Maximum number of items to collect in one run
    pub max_items: Option<usize>,
}
```

#### Running Garbage Collection

```rust
// Create a content store instance
let content_store = InMemoryContentStore::new();

// Configure garbage collection options
let options = GarbageCollectionOptions {
    // 30 days in milliseconds
    inactive_manifest_threshold_ms: 30 * 24 * 60 * 60 * 1000,
    // Run in dry-run mode first to see what would be deleted
    dry_run: true, 
    // Limit the number of items to collect (optional)
    max_items: Some(100),
};

// Run garbage collection
let result = content_store.run_garbage_collection(options).await?;

// Check results
println!("Would remove {} orphaned content blobs", result.content_blobs_removed);
println!("Would remove {} inactive manifests", result.manifests_removed);
println!("Would reclaim {} bytes", result.bytes_reclaimed);
```

## API

The primary APIs are:

* `ContentStorage` trait - Core interface for all implementations
* `Manifest` struct - Flow manifest data structure
* `ContentHash` - Type-safe wrapper for content hashes
* `GarbageCollectionOptions` - Configuration for garbage collection
* `GarbageCollectionResult` - Results of a garbage collection operation

## Implementation Details

Each implementation provides specific strategies for:

* Content storage and retrieval
* Manifest management
* Garbage collection

Refer to the code documentation for implementation-specific details.

*(Add details on configuration, choosing an implementation)*