use std::sync::Arc;

use crate::query::{
    DefaultQueryParser, Hit, HitPage, LocalScorer, ParseQuery, QueryEngine, QueryParser,
    QueryScorer, Reranker,
};
use serde_json::Value;

use crate::store::Store;
use crate::utils::hit::search_hit_to_hit;
use crate::{BoxFuture, Error, Result};

const DEFAULT_TOP: usize = 100;
const DEFAULT_SCORE_THRESHOLD: f32 = 0.2;
const DEFAULT_HYBRID_SCORE_WEIGHT: f32 = 0.95;

#[derive(Debug)]
pub struct DefaultQueryEngine {
    store: Arc<dyn Store>,
    query_parser: Arc<dyn QueryParser>,
    scorer: Arc<dyn QueryScorer>,
    reranker: Option<Arc<dyn Reranker>>,
    top: usize,
    score_threshold: f32,
    hybrid_score_weight: f32,
}

impl DefaultQueryEngine {
    #[must_use]
    pub fn new(store: Arc<dyn Store>, reranker: Option<Arc<dyn Reranker>>) -> Self {
        Self::with_query_parser(
            store,
            Arc::new(DefaultQueryParser),
            reranker,
            DEFAULT_TOP,
            DEFAULT_SCORE_THRESHOLD,
            DEFAULT_HYBRID_SCORE_WEIGHT,
        )
    }

    #[must_use]
    pub fn with_search_settings(
        store: Arc<dyn Store>,
        reranker: Option<Arc<dyn Reranker>>,
        top: usize,
        score_threshold: f32,
        hybrid_score_weight: f32,
    ) -> Self {
        Self::with_query_parser(
            store,
            Arc::new(DefaultQueryParser),
            reranker,
            top,
            score_threshold,
            hybrid_score_weight,
        )
    }

    #[must_use]
    pub fn with_query_parser(
        store: Arc<dyn Store>,
        query_parser: Arc<dyn QueryParser>,
        reranker: Option<Arc<dyn Reranker>>,
        top: usize,
        score_threshold: f32,
        hybrid_score_weight: f32,
    ) -> Self {
        Self::with_query_parser_and_scorer(
            store,
            query_parser,
            Arc::new(LocalScorer::default()),
            reranker,
            top,
            score_threshold,
            hybrid_score_weight,
        )
    }

    #[must_use]
    pub fn with_scorer(
        store: Arc<dyn Store>,
        scorer: Arc<dyn QueryScorer>,
        reranker: Option<Arc<dyn Reranker>>,
        top: usize,
        score_threshold: f32,
        hybrid_score_weight: f32,
    ) -> Self {
        Self::with_query_parser_and_scorer(
            store,
            Arc::new(DefaultQueryParser),
            scorer,
            reranker,
            top,
            score_threshold,
            hybrid_score_weight,
        )
    }

    #[must_use]
    pub fn with_query_parser_and_scorer(
        store: Arc<dyn Store>,
        query_parser: Arc<dyn QueryParser>,
        scorer: Arc<dyn QueryScorer>,
        reranker: Option<Arc<dyn Reranker>>,
        top: usize,
        score_threshold: f32,
        hybrid_score_weight: f32,
    ) -> Self {
        Self {
            store,
            query_parser,
            scorer,
            reranker,
            top,
            score_threshold,
            hybrid_score_weight,
        }
    }

    fn parse_query(&self, query: &str) -> Result<ParseQuery> {
        self.query_parser.parse(query)
    }

    async fn search_hits(&self, index_name: &str, body: Value) -> Result<Vec<Hit>> {
        let mut hits = self
            .store
            .search(index_name, body)
            .await?
            .into_iter()
            .map(|hit| search_hit_to_hit(hit, ""))
            .collect::<Result<Vec<_>>>()?;

        for hit in &mut hits {
            hit.scores.insert("hybrid_score".to_owned(), hit.score);
        }
        hits.sort_by(|left, right| right.score.total_cmp(&left.score));
        hits.truncate(self.top);
        Ok(hits)
    }

    async fn rerank(
        &self,
        parse_query: &ParseQuery,
        query_vector: Option<Vec<f32>>,
        mut hits: Vec<Hit>,
    ) -> Result<Vec<Hit>> {
        match self.reranker.as_ref() {
            Some(reranker) => {
                let model_scores = reranker.rerank(&parse_query.original_query, &hits).await?;
                if model_scores.len() != hits.len() {
                    return Err(Error::External(format!(
                        "reranker returned {} scores for {} hits",
                        model_scores.len(),
                        hits.len()
                    )));
                }

                for (hit, model_score) in hits.iter_mut().zip(model_scores) {
                    let term_score = self.scorer.term_score(&parse_query.keywords, hit);
                    let rerank_score = term_score * (1.0 - self.hybrid_score_weight)
                        + model_score * self.hybrid_score_weight;
                    hit.scores.insert("term_score".to_owned(), term_score);
                    hit.scores.insert("model_score".to_owned(), model_score);
                    hit.scores.insert("rerank_score".to_owned(), rerank_score);
                    hit.score = rerank_score;
                }
            }
            None => {
                for hit in &mut hits {
                    let term_score = self.scorer.term_score(&parse_query.keywords, hit);
                    let vector_score = query_vector
                        .as_deref()
                        .and_then(|vector| self.scorer.vector_score(vector, hit));
                    let rerank_score = match vector_score {
                        Some(vector_score) => {
                            hit.scores.insert("vector_score".to_owned(), vector_score);
                            term_score * (1.0 - self.hybrid_score_weight)
                                + vector_score * self.hybrid_score_weight
                        }
                        None => term_score,
                    };
                    hit.scores.insert("term_score".to_owned(), term_score);
                    hit.scores.insert("rerank_score".to_owned(), rerank_score);
                    hit.score = rerank_score;
                }
            }
        }

        hits.sort_by(|left, right| right.score.total_cmp(&left.score));
        Ok(hits)
    }

    fn filter_hits(&self, hits: Vec<Hit>) -> Vec<Hit> {
        if self.score_threshold <= 0.0 {
            return hits;
        }
        hits.into_iter()
            .filter(|hit| hit.score >= self.score_threshold)
            .collect()
    }

    fn paginate_hits(&self, hits: &[Hit], page_num: usize, page_size: usize) -> Vec<Hit> {
        let page_num = page_num.max(1);
        let page_size = page_size.max(1);
        let offset = (page_num - 1) * page_size;
        hits.iter().skip(offset).take(page_size).cloned().collect()
    }
}

impl QueryEngine for DefaultQueryEngine {
    fn search<'a>(
        &'a self,
        query: &'a str,
        index_name: &'a str,
        body: Value,
        page_num: Option<usize>,
        page_size: Option<usize>,
    ) -> BoxFuture<'a, Result<HitPage>> {
        Box::pin(async move {
            let page_num = page_num.unwrap_or(1).max(1);
            let page_size = page_size.unwrap_or(10).max(1);

            tracing::info!(query = %query, "search.start");
            let parse_query = self.parse_query(query)?;
            let query_vector = self.scorer.query_vector(&body);
            let hits = self.search_hits(index_name, body).await?;
            let ranked_hits = self.rerank(&parse_query, query_vector, hits).await?;
            let filtered_hits = self.filter_hits(ranked_hits);
            let page_hits = self.paginate_hits(&filtered_hits, page_num, page_size);

            Ok(HitPage {
                total: filtered_hits.len(),
                hits: page_hits,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use serde_json::{Value, json};

    use crate::index::{DefaultChunk, Positions};
    use crate::query::{Hit, QueryEngine, Reranker};
    use crate::store::{Item, SearchHit, Store};
    use crate::{BoxFuture, Result};

    use super::{
        DEFAULT_HYBRID_SCORE_WEIGHT, DEFAULT_SCORE_THRESHOLD, DEFAULT_TOP, DefaultQueryEngine,
    };

    #[tokio::test]
    async fn query_engine_search_uses_mock_store() {
        let engine =
            DefaultQueryEngine::with_search_settings(Arc::new(MockStore), None, 1024, 0.0, 0.0);

        let page = engine
            .search(
                "怎么重置密码",
                "chunks",
                search_body("怎么重置密码", &[]),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.hits[0].id, "chunk_1");
    }

    #[test]
    fn query_engine_new_uses_default_search_settings() {
        let engine = DefaultQueryEngine::new(Arc::new(MockStore), None);

        assert_eq!(engine.top, DEFAULT_TOP);
        assert_eq!(engine.score_threshold, DEFAULT_SCORE_THRESHOLD);
        assert_eq!(engine.hybrid_score_weight, DEFAULT_HYBRID_SCORE_WEIGHT);
    }

    #[tokio::test]
    async fn query_engine_passes_caller_built_request_to_store() {
        let store = Arc::new(RecordingStore::default());
        let engine = DefaultQueryEngine::new(store.clone(), None);
        let body = search_body(
            "怎么重置密码",
            &[json!({ "terms": { "knowledge_base_id": ["kb_1"] } })],
        );
        let _ = engine
            .search("怎么重置密码", "custom_chunks", body.clone(), None, None)
            .await
            .unwrap();

        let recorded = store.recorded.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "custom_chunks");
        assert_eq!(recorded[0].1, body);
    }

    #[tokio::test]
    async fn query_engine_treats_body_as_single_backend_condition() {
        let engine =
            DefaultQueryEngine::with_search_settings(Arc::new(RouteStore), None, 1024, 0.0, 0.95);
        let page = engine
            .search(
                "怎么重置密码",
                "chunks",
                json!({ "mode": "hybrid" }),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(page.total, 1);
        assert!(page.hits[0].scores.contains_key("hybrid_score"));
    }

    #[tokio::test]
    async fn query_engine_local_rerank_uses_query_vector_from_request() {
        let engine =
            DefaultQueryEngine::with_search_settings(Arc::new(VectorStore), None, 1024, 0.0, 0.95);
        let page = engine
            .search(
                "怎么重置密码",
                "chunks",
                json!({
                    "query": { "match_all": {} },
                    "knn": {
                        "field": "embedding",
                        "query_vector": [1.0, 0.0],
                        "k": 2
                    }
                }),
                None,
                Some(2),
            )
            .await
            .unwrap();

        assert_eq!(page.hits[0].id, "chunk_1");
        assert_eq!(page.hits[0].scores.get("vector_score").copied(), Some(1.0));
        assert!(page.hits[0].score > page.hits[1].score);
    }

    #[tokio::test]
    async fn query_engine_accepts_explicit_pagination_without_params_struct() {
        let engine = DefaultQueryEngine::new(Arc::new(MockStore), None);

        let page = engine
            .search(
                "怎么重置密码",
                "chunks",
                search_body("怎么重置密码", &[]),
                None,
                Some(1),
            )
            .await
            .unwrap();

        assert_eq!(page.hits.len(), 1);
    }

    #[tokio::test]
    async fn query_engine_rejects_wrong_reranker_score_count() {
        let engine = DefaultQueryEngine::new(Arc::new(MockStore), Some(Arc::new(BadReranker)));

        let error = engine
            .search(
                "怎么重置密码",
                "chunks",
                search_body("怎么重置密码", &[]),
                None,
                None,
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("reranker returned 0 scores"));
    }

    #[derive(Debug)]
    struct MockStore;

    impl Store for MockStore {
        fn create_schema<'a>(
            &'a self,
            _index_name: &'a str,
            _schema: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn insert<'a>(&'a self, _index_name: &'a str, _item: Item) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn batch_insert<'a>(
            &'a self,
            _index_name: &'a str,
            _items: Vec<Item>,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn update<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
            _fields: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(&'a self, _index_name: &'a str, _query: Value) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn get<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
        ) -> BoxFuture<'a, Result<Option<Value>>> {
            Box::pin(async { Ok(None) })
        }

        fn search<'a>(
            &'a self,
            _index_name: &'a str,
            _body: Value,
        ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
            Box::pin(async {
                let mut scores = BTreeMap::new();
                scores.insert("text".to_owned(), 1.0);
                let chunk = default_chunk();
                Ok(vec![SearchHit {
                    id: chunk.id.clone(),
                    source: serde_json::to_value(chunk)?,
                    score: 1.0,
                    scores,
                    highlight: None,
                }])
            })
        }
    }

    #[derive(Debug)]
    struct BadReranker;

    impl Reranker for BadReranker {
        fn rerank<'a>(
            &'a self,
            _query: &'a str,
            _hits: &'a [Hit],
        ) -> BoxFuture<'a, Result<Vec<f32>>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    #[derive(Debug, Default)]
    struct RecordingStore {
        recorded: Mutex<Vec<(String, Value)>>,
    }

    impl Store for RecordingStore {
        fn create_schema<'a>(
            &'a self,
            _index_name: &'a str,
            _schema: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn insert<'a>(&'a self, _index_name: &'a str, _item: Item) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn batch_insert<'a>(
            &'a self,
            _index_name: &'a str,
            _items: Vec<Item>,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn update<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
            _fields: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(&'a self, _index_name: &'a str, _query: Value) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn get<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
        ) -> BoxFuture<'a, Result<Option<Value>>> {
            Box::pin(async { Ok(None) })
        }

        fn search<'a>(
            &'a self,
            index_name: &'a str,
            body: Value,
        ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
            self.recorded
                .lock()
                .unwrap()
                .push((index_name.to_owned(), body));
            Box::pin(async { Ok(vec![search_hit(1.0)]) })
        }
    }

    #[derive(Debug)]
    struct RouteStore;

    impl Store for RouteStore {
        fn create_schema<'a>(
            &'a self,
            _index_name: &'a str,
            _schema: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn insert<'a>(&'a self, _index_name: &'a str, _item: Item) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn batch_insert<'a>(
            &'a self,
            _index_name: &'a str,
            _items: Vec<Item>,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn update<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
            _fields: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(&'a self, _index_name: &'a str, _query: Value) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn get<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
        ) -> BoxFuture<'a, Result<Option<Value>>> {
            Box::pin(async { Ok(None) })
        }

        fn search<'a>(
            &'a self,
            _index_name: &'a str,
            body: Value,
        ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
            Box::pin(async move {
                let score = match body.get("mode").and_then(Value::as_str) {
                    Some("hybrid") => 0.9,
                    _ => 0.8,
                };
                Ok(vec![search_hit(score)])
            })
        }
    }

    #[derive(Debug)]
    struct VectorStore;

    impl Store for VectorStore {
        fn create_schema<'a>(
            &'a self,
            _index_name: &'a str,
            _schema: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn insert<'a>(&'a self, _index_name: &'a str, _item: Item) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn batch_insert<'a>(
            &'a self,
            _index_name: &'a str,
            _items: Vec<Item>,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn update<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
            _fields: Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(&'a self, _index_name: &'a str, _query: Value) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn get<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
        ) -> BoxFuture<'a, Result<Option<Value>>> {
            Box::pin(async { Ok(None) })
        }

        fn search<'a>(
            &'a self,
            _index_name: &'a str,
            _body: Value,
        ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
            Box::pin(async {
                let mut first = default_chunk();
                first.id = "chunk_1".to_owned();
                first.embedding = Some(vec![1.0, 0.0]);

                let mut second = default_chunk();
                second.id = "chunk_2".to_owned();
                second.embedding = Some(vec![0.0, 1.0]);

                Ok(vec![
                    SearchHit {
                        id: first.id.clone(),
                        source: serde_json::to_value(first)?,
                        score: 0.1,
                        scores: BTreeMap::new(),
                        highlight: None,
                    },
                    SearchHit {
                        id: second.id.clone(),
                        source: serde_json::to_value(second)?,
                        score: 10.0,
                        scores: BTreeMap::new(),
                        highlight: None,
                    },
                ])
            })
        }
    }

    fn search_hit(score: f32) -> SearchHit {
        let chunk = default_chunk();
        SearchHit {
            id: chunk.id.clone(),
            source: serde_json::to_value(chunk).unwrap(),
            score,
            scores: BTreeMap::new(),
            highlight: None,
        }
    }

    fn default_chunk() -> DefaultChunk {
        DefaultChunk {
            id: "chunk_1".to_owned(),
            tenant_id: None,
            user_id: None,
            knowledge_base_id: None,
            doc_id: "doc_1".to_owned(),
            title: "密码手册".to_owned(),
            title_tokens: vec!["密码手册".to_owned()],
            content: "点击忘记密码完成重置".to_owned(),
            content_tokens: vec!["点击忘记密码完成重置".to_owned()],
            keywords: vec!["密码".to_owned()],
            keyword_tokens: vec!["密码".to_owned()],
            questions: vec!["怎么重置密码".to_owned()],
            question_tokens: vec!["怎么重置密码".to_owned()],
            embedding: None,
            positions: Positions::default(),
            tags: Vec::new(),
            images: Vec::new(),
        }
    }

    fn search_body(text: &str, filters: &[Value]) -> Value {
        json!({
            "query": {
                "bool": {
                    "must": [{
                        "query_string": {
                            "query": text,
                            "fields": [
                                "questions^8",
                                "keywords^6",
                                "title^4",
                                "content^1",
                                "question_tokens^3",
                                "keyword_tokens^3",
                                "title_tokens^2",
                                "content_tokens^1"
                            ],
                            "default_operator": "OR"
                        }
                    }],
                    "filter": filters
                }
            },
            "from": 0,
            "size": 1024
        })
    }
}
