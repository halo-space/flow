## Why

The project is being rebuilt as a Rust 1.95 RAG search library with clear boundaries between data access, index construction, and query execution. The current directory has been cleaned so the implementation can follow the existing `doc/search` design instead of inheriting unrelated Python project structure.

## What Changes

- Create a Rust 2024 workspace/library for RAG search.
- Introduce base traits for `Store`, `IndexBuilder`, and `QueryEngine` so concrete backends and strategies can be swapped cleanly.
- Provide an Elasticsearch-backed `Store` implementation as the first backend.
- Define default document/chunk models, plus generic store items, search requests, hits, scores, and responses.
- Implement the query-side pipeline around query preparation, caller-built recall, rerank hooks, filtering, pagination, and response assembly.
- Implement the index-building pipeline around extract, parse, chunk, tokenize, embed, and Store writes.
- Use Rust 1.95 / Rust 2024 style, including modern async callback patterns where useful, while keeping stable components as traits.

## Capabilities

### New Capabilities

- `store-core`: Data access contracts, generic item models, CRUD, and caller-built search execution.
- `elastic-store`: Elasticsearch implementation of the `Store` trait for caller-selected indexes.
- `index-builder`: Document ingestion pipeline for extracting content, parsing formats, chunking, tokenizing, embedding, and indexing.
- `engine-core`: Query execution pipeline for query preparation, recall, rerank, filtering, pagination, and response assembly.

### Modified Capabilities

None.

## Impact

- Adds Rust project files: `Cargo.toml`, `src/`, and supporting tests.
- Adds OpenSpec artifacts under `openspec/changes/rag-search-core/`.
- Uses dependencies for async runtime, serialization, errors, logging, and the official Elasticsearch SDK.
- Establishes Elasticsearch as the first concrete search backend while keeping the architecture backend-extensible.
