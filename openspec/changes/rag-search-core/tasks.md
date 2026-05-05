## 1. Project Setup

- [x] 1.1 Create Rust 2024 library package metadata and dependency configuration.
- [x] 1.2 Add public module layout for core models, traits, store, index builder, engine, and utilities.
- [x] 1.3 Update gitignore for Rust build outputs and local environment files.

## 2. Core Models and Traits

- [x] 2.1 Define shared error and result types.
- [x] 2.2 Define default document/chunk models and generic item, hit, score, request, and response models.
- [x] 2.3 Define `Store`, `IndexBuilder`, `QueryEngine`, `Embedder`, `Reranker`, `Tokenizer`, `Extractor`, `Parser`, and `Chunker` traits.
- [x] 2.4 Keep engine strategy values as direct constructor parameters and keep index builder strategy values configurable.

## 3. Store Implementation

- [x] 3.1 Keep store requests as caller-built backend query bodies.
- [x] 3.2 Implement `Elastic` SDK client structure and CRUD methods.
- [x] 3.3 Implement Elasticsearch schema creation through caller-provided mappings.
- [x] 3.4 Execute caller-built Elasticsearch query bodies in `Elastic`.

## 4. Index Builder Implementation

- [x] 4.1 Implement default extractor for raw text inputs.
- [x] 4.2 Implement text and QA parsers.
- [x] 4.3 Implement fixed, delimiter, and simple semantic chunking.
- [x] 4.4 Implement simple tokenizer and optional embedding integration.
- [x] 4.5 Implement `DefaultIndexBuilder::build` and `DefaultIndexBuilder::index`.

## 5. QueryEngine Implementation

- [x] 5.1 Implement query preparation without business DSL construction.
- [x] 5.2 Execute caller-built base/vector recall requests through `Store`.
- [x] 5.3 Implement recall hit assembly and score recording.
- [x] 5.4 Implement external rerank hook and local rerank fallback.
- [x] 5.5 Implement score filtering, pagination, and response assembly.

## 6. Validation

- [x] 6.1 Add unit tests for tokenizer, query preparation, chunking, and recall handling.
- [x] 6.2 Add mock store tests for engine flow.
- [x] 6.3 Run `cargo fmt` and `cargo check`.
- [x] 6.4 Validate OpenSpec change artifacts.
