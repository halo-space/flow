## Context

The `rag` repository has been cleaned and is being rebuilt as a Rust 1.95 / Rust 2024 RAG search library. The target design follows the existing `doc/search` architecture: `Store` owns data access, `IndexBuilder` owns ingestion/building, and `QueryEngine` owns query execution.

The first concrete backend is Elasticsearch. Other backends must be possible later, so the core layer must define stable traits and data models before implementing Elasticsearch-specific code.

## Goals / Non-Goals

**Goals:**

- Provide base traits for `Store`, `IndexBuilder`, and `QueryEngine`.
- Provide default document/chunk models plus generic item, hit, request, and response models.
- Implement an Elasticsearch-backed `Store` for CRUD and execution of caller-built search bodies.
- Implement an `IndexBuilder` pipeline that can extract raw text, parse formats, chunk content, tokenize chunks, embed chunks, and write through `Store`.
- Implement a `QueryEngine` pipeline that prepares query text, executes caller-built recall requests, reranks, filters, paginates, and builds responses.
- Use Rust 1.95 style where it improves clarity while keeping long-lived components as traits.

**Non-Goals:**

- No object storage integration in the first version.
- No production-grade document parsers beyond plain text and simple QA parsing in the first version.
- No rerank logic in `Store`; ranking beyond backend scoring stays in `QueryEngine`.
- No complex Observer/Trace/status subsystem; structured logs and errors are enough.
- No distributed ingestion scheduler or background task orchestration.

## Decisions

### Trait-first architecture

`Store`, `IndexBuilder`, and `QueryEngine` are base traits. Concrete implementations are separate structs such as `Elastic`, `DefaultIndexBuilder`, and `DefaultQueryEngine`.

Rationale: this keeps the public contracts stable while allowing Elasticsearch, OpenSearch, local mock stores, remote embedding services, and rerankers to be swapped independently.

Alternative considered: concrete structs only. Rejected because backend and model replacement would leak implementation details through the codebase.

### Store executes caller-built backend queries

`Store::search()` receives an index name and an already-built backend query body. It does not construct business filters, text queries, vector queries, weighted fusion, rerank, or multi-route orchestration.

Rationale: `Store` is a data access boundary. Query DSL construction belongs to caller/business code, while strategy-level ranking belongs to `QueryEngine`.

Alternative considered: model generic text/vector/filter conditions in `Store`. Rejected because backend DSL construction is business-specific and differs too much across stores.

### Document and chunk schemas are defaults, not framework contracts

`index::DefaultDocument` and `index::DefaultChunk` are convenience models used by the first `DefaultIndexBuilder`. They live under the `index` module because they belong to the default ingestion/building path, not to the global framework contract. They are not mandatory business schemas. `Store` writes generic `Item { id, source }`, and `QueryEngine` returns generic `Hit { id, source, score, scores, highlight }`, so callers can define their own document/chunk fields and index mappings.

Rationale: document and chunk fields often include product-specific concepts such as tenant, user, permissions, knowledge base, parser metadata, file location, tags, images, table coordinates, or domain fields. The framework should not pretend those fields are universal.

Alternative considered: export `Document` and `Chunk` as canonical models. Rejected because it would make business fields look like framework requirements.

### QueryEngine executes provided recall requests

`QueryEngine::search()` receives the user query, one required caller-built search request, and optional `page_num` / `page_size` values. The default engine does not build Elasticsearch `multi_match`, `knn`, bool filters, tenant filters, knowledge-base filters, or other business DSL in production code.

Rationale: `QueryEngine` should own framework-level query flow—query preparation, recall coordination, rerank, threshold filtering, pagination, and response assembly—without pretending to know every product's query shape.

Alternative considered: hardcode Elasticsearch text/vector request builders in `DefaultQueryEngine`. Rejected because it would turn framework code into incomplete business code. Elasticsearch DSL examples belong in tests or business adapters.

### QueryEngine prepares query text before ranking

Query preparation follows a stable pipeline: preserve the original query for tracing, add Chinese/English boundary spacing, normalize case and character width, normalize traditional Chinese to simplified Chinese, remove unsafe search punctuation, remove weak semantic words, choose Chinese or English token handling, compute weighted terms, expand synonyms, use controlled fine-grained token expansion, and produce keywords for rerank/explanation.

Rationale: query preparation affects local rerank and explainability even when the backend request body is caller-built. Keeping this logic in `QueryEngine` gives consistent ranking behavior without forcing `QueryEngine` to own backend DSL construction.

Alternative considered: let every caller prepare query keywords independently. Rejected because it would make local rerank behavior inconsistent and harder to test.

### QueryEngine owns recall coordination and ranking

`QueryEngine::search_hits()` coordinates the caller-built recall request and writes recall details to `Hit.scores`.

Rationale: this keeps query orchestration in `QueryEngine` while leaving backend DSL and transport details in the caller and `Store`.

Alternative considered: expose multiple stage-specific search entry points. Rejected because a single caller-built request is easier to reason about and keeps the API surface smaller.

### Long-lived components use traits; temporary async callbacks use AsyncFn

`Store`, `Embedder`, `Reranker`, `Tokenizer`, `Extractor`, `Parser`, and `Chunker` are traits. Utility operations such as retry hooks may use `AsyncFn` / `AsyncFnMut` when useful.

Rationale: traits make dependency injection and testing simple. AsyncFn avoids unnecessary `Box<dyn Future>` for short-lived callbacks.

Alternative considered: use callbacks everywhere. Rejected because the project would become callback-heavy and harder to navigate.

### Elasticsearch implementation uses the official SDK

`Elastic` uses the official `elasticsearch-rs` SDK and sends caller-built JSON DSL bodies.

Rationale: this keeps ES transport details inside the ES backend while preventing business query construction from leaking into the data access layer.

Alternative considered: raw `reqwest` HTTP calls. Rejected because the official SDK provides typed API entry points while still allowing JSON DSL bodies.

### Logs and retry stay lightweight

The code uses `tracing` events for important stages. Retry is only used by callers or specific external integrations; `Store` does not hide persistent failures.

Rationale: first version should be debuggable without creating a separate trace/status subsystem.

## Risks / Trade-offs

- Elasticsearch DSL drift → Keep query JSON construction in caller/business code and keep `Elastic` limited to transport execution.
- Vector search mapping differences between ES versions → Provide mappings through `create_schema` from the caller.
- Tokenization quality affects recall → Provide a trait and a simple default tokenizer, so better tokenizers can replace it later.
- Local rerank is heuristic → Keep external reranker as a trait, keep local scoring behind `QueryScorer`, and record score details in `Hit.scores`.
- Async trait object overhead → Accept it for stable components; use `AsyncFn` for short-lived callback helpers where static dispatch matters.

## Migration Plan

1. Keep the existing Rust style document under `docs/`.
2. Add OpenSpec specs and implementation tasks.
3. Add Rust library skeleton and core traits.
4. Implement in-memory-safe default pipelines and Elasticsearch-backed store.
5. Add focused unit tests for query preparation, indexing pipeline, and response behavior.
6. Run formatting and compile checks.

Rollback is simple during initial development: remove the new Rust files and OpenSpec change before archiving.

## Open Questions

- Which production tokenizer should replace the first simple tokenizer?
- Which embedding/rerank service protocol should be the default remote implementation?
- Whether Elasticsearch dense vector mapping should target a specific ES minor version after integration testing.
