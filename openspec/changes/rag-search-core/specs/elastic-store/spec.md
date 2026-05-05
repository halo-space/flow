## ADDED Requirements

### Requirement: Elastic implements Store
The system SHALL provide an `Elastic` implementation of the `Store` trait using the official Elasticsearch Rust SDK.

#### Scenario: Insert record through Elastic
- **WHEN** a record is inserted through `Elastic`
- **THEN** Elasticsearch receives an index request for the provided index name and record id

### Requirement: Elastic creates schemas from caller mappings
The system SHALL create Elasticsearch indexes using schema bodies provided by the caller.

#### Scenario: Create index schema
- **WHEN** `create_schema` is called with an index name and mapping body
- **THEN** Elastic sends that mapping body to Elasticsearch without adding business fields

### Requirement: Elastic executes caller-built query bodies
The system SHALL execute search requests using query bodies provided by the caller.

#### Scenario: Execute search body
- **WHEN** `Store::search` is called with an index name and JSON body
- **THEN** Elastic sends the body to Elasticsearch without translating business filters or relevance conditions
