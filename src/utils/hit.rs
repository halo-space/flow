use crate::Result;
use crate::query::Hit;
use crate::store::SearchHit;

pub fn search_hit_to_hit(mut hit: SearchHit, score_key: &str) -> Result<Hit> {
    if !score_key.is_empty() {
        hit.scores.insert(score_key.to_owned(), hit.score);
    }
    Ok(Hit {
        id: hit.id,
        source: hit.source,
        score: hit.score,
        scores: hit.scores,
        highlight: hit.highlight,
    })
}
