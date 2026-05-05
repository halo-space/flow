## ADDED Requirements

### Requirement: QueryEngine exposes query search
The system SHALL define a `QueryEngine` trait that accepts a user query, one required `index_name`, one required caller-built search body, and optional `page_num` / `page_size`, then returns a `HitPage`.

#### Scenario: Simple query
- **WHEN** caller searches with a query string, index name, and caller-built search body
- **THEN** QueryEngine uses default pagination and returns hits

### Requirement: QueryEngine prepares query keywords
The system SHALL preserve the original query, add Chinese/English boundary spacing, lowercase English, convert fullwidth characters to halfwidth, normalize traditional Chinese to simplified Chinese, replace unsafe search syntax characters with spaces, remove weak semantic words, choose Chinese or English token handling, tokenize, compute term weights, expand synonyms, apply controlled fine-grained token expansion, generate keywords, and generate a text expression.

#### Scenario: Mixed Chinese and English query
- **WHEN** query contains mixed Chinese and English tokens
- **THEN** QueryEngine adds language boundary spacing and prepares stable text tokens suitable for rerank and explanation

#### Scenario: Query includes weak words and search syntax characters
- **WHEN** query contains question words, weak words, or backend search syntax characters
- **THEN** QueryEngine keeps the original query for tracing while preparing cleaned keywords for rerank and explanation

### Requirement: QueryEngine does not build business query DSL
The default QueryEngine implementation SHALL NOT construct backend-specific text DSL, vector DSL, bool filters, tenant filters, knowledge-base filters, or document filters in production code.

#### Scenario: Business needs custom filters
- **WHEN** caller needs knowledge-base, tenant, permission, highlight, or field-weight logic
- **THEN** caller builds those details into the body before calling QueryEngine

### Requirement: QueryEngine coordinates recall
The system SHALL use `search_hits` as the coordinator for recall execution and score recording.

#### Scenario: Recall request exists
- **WHEN** caller provides an index name and caller-built search body
- **THEN** QueryEngine runs them through Store and records the backend score as `hybrid_score`

### Requirement: QueryEngine reranks and paginates results
The system SHALL rerank hits using external reranker when available or configurable local query scoring otherwise, then filter by score and paginate.

#### Scenario: Reranker unavailable
- **WHEN** no external reranker is configured
- **THEN** QueryEngine uses the configured `QueryScorer` based on available query tokens, hit content, and optional query vectors
