## ADDED Requirements

### Requirement: IndexBuilder exposes build and index operations
The system SHALL define an `IndexBuilder` trait with `build` for returning document/chunk-shaped items and `index` for writing chunk search records through Store.

#### Scenario: Build without writing
- **WHEN** caller invokes `build`
- **THEN** the system returns a document and chunks without requiring Store writes

### Requirement: IndexBuilder runs ordered build stages
The system SHALL run ingestion stages in order: extract, parse, chunk, build document, build chunks, tokenize, embed, and index.

#### Scenario: Index text document
- **WHEN** caller indexes a text document
- **THEN** content is extracted, parsed, chunked, tokenized, optionally embedded, and written as chunk search records while the built document is returned for caller-managed storage

### Requirement: IndexBuilder supports text and QA formats
The system SHALL support plain text format and simple QA format in the first implementation.

#### Scenario: QA input is parsed
- **WHEN** content contains question and answer pairs in QA format
- **THEN** each pair can become a chunk with answer content and question metadata

### Requirement: IndexBuilder writes chunk search records
The system SHALL write only chunk search records during indexing and return the built document object for caller-managed document storage.

#### Scenario: Chunk is generated
- **WHEN** a chunk is built
- **THEN** it contains its own content, positions, tokens, optional embedding, and document reference fields

### Requirement: Default document and chunk models are optional
The system SHALL expose `DefaultDocument` and `DefaultChunk` from the `index` module as default output shapes, not required framework schemas.

#### Scenario: Caller uses custom schema
- **WHEN** a caller implements a custom `IndexBuilder` or transforms build output before indexing
- **THEN** Store accepts generic items as long as each item has an id and serialized source
