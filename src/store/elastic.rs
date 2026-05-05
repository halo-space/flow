use elasticsearch::{
    BulkOperation, BulkParts, DeleteByQueryParts, Elasticsearch, GetParts, IndexParts, SearchParts,
    UpdateParts,
    http::{StatusCode, transport::Transport},
    indices::IndicesCreateParts,
};
use serde_json::{Value, json};

use crate::store::{Item, SearchHit, Store};
use crate::{BoxFuture, Error, Result};

#[derive(Debug, Clone)]
pub struct Elastic {
    client: Elasticsearch,
}

impl Elastic {
    pub fn new(urls: &str) -> Result<Self> {
        let nodes: Vec<&str> = urls
            .split(',')
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .collect();
        if nodes.is_empty() {
            return Err(Error::InvalidInput(
                "elastic urls cannot be empty".to_owned(),
            ));
        }

        let transport = if nodes.len() == 1 {
            Transport::single_node(nodes[0])
        } else {
            Transport::static_node_list(nodes)
        }
        .map_err(|error| Error::Store(format!("invalid elasticsearch transport: {error}")))?;
        Ok(Self {
            client: Elasticsearch::new(transport),
        })
    }
}

impl Store for Elastic {
    fn create_schema<'a>(
        &'a self,
        index_name: &'a str,
        schema: Value,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let response = self
                .client
                .indices()
                .create(IndicesCreateParts::Index(index_name))
                .body(schema)
                .send()
                .await?;
            ensure_success(response.status_code(), "create_schema")
        })
    }

    fn insert<'a>(&'a self, index_name: &'a str, item: Item) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let response = self
                .client
                .index(IndexParts::IndexId(index_name, &item.id))
                .body(item.source)
                .send()
                .await?;
            ensure_success(response.status_code(), "insert")
        })
    }

    fn batch_insert<'a>(
        &'a self,
        index_name: &'a str,
        items: Vec<Item>,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if items.is_empty() {
                return Ok(());
            }

            let operations: Vec<BulkOperation<Value>> = items
                .into_iter()
                .map(|item| {
                    BulkOperation::index(item.source)
                        .index(index_name)
                        .id(&item.id)
                        .into()
                })
                .collect();

            let response = self
                .client
                .bulk(BulkParts::None)
                .body(operations)
                .send()
                .await?;
            let status = response.status_code();
            let value: Value = response.json().await?;
            ensure_success(status, "batch_insert")?;
            ensure_bulk_success(value)
        })
    }

    fn update<'a>(
        &'a self,
        index_name: &'a str,
        id: &'a str,
        fields: Value,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let response = self
                .client
                .update(UpdateParts::IndexId(index_name, id))
                .body(json!({ "doc": fields }))
                .send()
                .await?;
            ensure_success(response.status_code(), "update")
        })
    }

    fn delete<'a>(&'a self, index_name: &'a str, query: Value) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let response = self
                .client
                .delete_by_query(DeleteByQueryParts::Index(&[index_name]))
                .body(query)
                .send()
                .await?;
            ensure_success(response.status_code(), "delete")
        })
    }

    fn get<'a>(&'a self, index_name: &'a str, id: &'a str) -> BoxFuture<'a, Result<Option<Value>>> {
        Box::pin(async move {
            let response = self
                .client
                .get(GetParts::IndexId(index_name, id))
                .send()
                .await?;

            if response.status_code() == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let status = response.status_code();
            let value: Value = response.json().await?;
            ensure_success(status, "get")?;
            let Some(source) = value.get("_source") else {
                return Ok(None);
            };

            Ok(Some(source.clone()))
        })
    }

    fn search<'a>(
        &'a self,
        index_name: &'a str,
        body: Value,
    ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
        Box::pin(async move {
            let response = self
                .client
                .search(SearchParts::Index(&[index_name]))
                .body(body)
                .send()
                .await?;
            let status = response.status_code();
            let value: Value = response.json().await?;
            ensure_success(status, "search")?;
            parse_hits(value)
        })
    }
}

fn ensure_success(status: StatusCode, action: &str) -> Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        Err(Error::Store(format!(
            "elastic {action} failed with status {status}"
        )))
    }
}

fn parse_hits(value: Value) -> Result<Vec<SearchHit>> {
    let hits = value
        .pointer("/hits/hits")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    hits.into_iter()
        .map(|hit| {
            let score = hit
                .get("_score")
                .and_then(Value::as_f64)
                .unwrap_or_default() as f32;
            let source = hit
                .get("_source")
                .cloned()
                .ok_or_else(|| Error::Store("missing _source in search hit".to_owned()))?;
            let id = hit
                .get("_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    source
                        .get("id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_default();
            let scores = std::collections::BTreeMap::new();
            let highlight = parse_highlight(hit.get("highlight"));
            Ok(SearchHit {
                id,
                source,
                score,
                scores,
                highlight,
            })
        })
        .collect()
}

fn ensure_bulk_success(value: Value) -> Result<()> {
    if value
        .get("errors")
        .and_then(Value::as_bool)
        .unwrap_or_default()
    {
        return Err(Error::Store(
            "elastic batch_insert has item errors".to_owned(),
        ));
    }

    Ok(())
}

fn parse_highlight(value: Option<&Value>) -> Option<String> {
    let object = value?.as_object()?;
    let fragments = object
        .values()
        .filter_map(Value::as_array)
        .flat_map(|items| items.iter().filter_map(Value::as_str))
        .collect::<Vec<_>>();

    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Elastic, ensure_bulk_success, parse_hits};

    #[test]
    fn rejects_empty_urls() {
        assert!(Elastic::new(" , ").is_err());
    }

    #[test]
    fn parses_highlight_fragments() {
        let hits = parse_hits(json!({
            "hits": {
                "hits": [{
                    "_id": "chunk_1",
                    "_score": 0.9,
                    "_source": { "id": "chunk_1", "content": "hello" },
                    "highlight": {
                        "content": ["<em>hello</em>"],
                        "title": ["title"]
                    }
                }]
            }
        }))
        .unwrap();

        assert_eq!(hits[0].highlight.as_deref(), Some("<em>hello</em>\ntitle"));
    }

    #[test]
    fn rejects_bulk_item_errors() {
        let error = ensure_bulk_success(json!({ "errors": true })).unwrap_err();

        assert!(error.to_string().contains("batch_insert has item errors"));
    }
}
