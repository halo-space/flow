## ADDED Requirements

### Requirement: Store exposes CRUD operations
The system SHALL define a `Store` trait for creating schemas, inserting records, batch inserting records, updating records by id, deleting records by caller-provided query body, getting records by id, and searching records by caller-provided query body.

#### Scenario: Store implementation is called through trait
- **WHEN** application code receives a `dyn Store`
- **THEN** it can perform CRUD operations without knowing the concrete backend

### Requirement: Store search accepts caller-built backend query body
The system SHALL make each `Store::search` call execute a backend query body constructed by the caller.

#### Scenario: QueryEngine needs hybrid recall
- **WHEN** the query flow needs both text and vector recall
- **THEN** caller provides one backend query body that represents the desired text, vector, or hybrid search, and QueryEngine calls `Store::search`

### Requirement: Store does not own business query construction
The system SHALL NOT model business filters, text relevance, vector relevance, or should clauses inside Store.

#### Scenario: Search within knowledge bases
- **WHEN** a query specifies knowledge base ids and text content
- **THEN** caller/business code builds the backend query body before calling Store or QueryEngine

### Requirement: Store returns hits with score details
The system SHALL return search hits containing raw source fields, a current `score`, and a `scores` map for caller-level explanation values.

#### Scenario: Text search returns a hit
- **WHEN** Store performs text search
- **THEN** Store returns backend hits and the QueryEngine layer may label route-specific scores
