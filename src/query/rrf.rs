use std::collections::BTreeMap;

use crate::query::Hit;

#[must_use]
pub fn fuse_by_rrf(
    text_hits: Vec<Hit>,
    vector_hits: Vec<Hit>,
    text_weight: f32,
    vector_weight: f32,
    rrf_k: usize,
    top: usize,
) -> Vec<Hit> {
    let mut merged: BTreeMap<String, Hit> = BTreeMap::new();
    let rrf_k = rrf_k as f32;

    for (rank, mut hit) in text_hits.into_iter().enumerate() {
        let contribution = text_weight / (rrf_k + rank as f32 + 1.0);
        hit.scores.insert("text_rank".to_owned(), rank as f32 + 1.0);
        hit.scores.insert("rrf".to_owned(), contribution);
        hit.scores.insert("hybrid_score".to_owned(), contribution);
        merged.insert(hit.id.clone(), hit);
    }

    for (rank, hit) in vector_hits.into_iter().enumerate() {
        let contribution = vector_weight / (rrf_k + rank as f32 + 1.0);
        let id = hit.id.clone();

        match merged.get_mut(&id) {
            Some(existing) => {
                let vector_score = hit.scores.get("vector").copied().unwrap_or(hit.score);
                existing.scores.insert("vector".to_owned(), vector_score);
                existing
                    .scores
                    .insert("vector_rank".to_owned(), rank as f32 + 1.0);
                let rrf = existing.scores.get("rrf").copied().unwrap_or_default() + contribution;
                existing.scores.insert("rrf".to_owned(), rrf);
                existing.scores.insert("hybrid_score".to_owned(), rrf);
                existing.score = rrf;
            }
            None => {
                let mut hit = hit;
                hit.scores
                    .insert("vector_rank".to_owned(), rank as f32 + 1.0);
                hit.scores.insert("rrf".to_owned(), contribution);
                hit.scores.insert("hybrid_score".to_owned(), contribution);
                hit.score = contribution;
                merged.insert(id, hit);
            }
        }
    }

    let mut hits: Vec<Hit> = merged.into_values().collect();
    hits.sort_by(|left, right| right.score.total_cmp(&left.score));
    hits.truncate(top);
    hits
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::fuse_by_rrf;
    use crate::query::Hit;

    fn hit(id: &str, score: f32, key: &str) -> Hit {
        let mut scores = BTreeMap::new();
        scores.insert(key.to_owned(), score);
        Hit {
            id: id.to_owned(),
            source: serde_json::json!({ "id": id, "content": "content" }),
            score,
            scores,
            highlight: None,
        }
    }

    #[test]
    fn rrf_merges_same_chunk() {
        let hits = fuse_by_rrf(
            vec![hit("a", 0.8, "text")],
            vec![hit("a", 0.9, "vector")],
            0.7,
            0.3,
            60,
            10,
        );
        assert_eq!(hits.len(), 1);
        assert!(hits[0].scores.contains_key("text"));
        assert!(hits[0].scores.contains_key("vector"));
    }
}
