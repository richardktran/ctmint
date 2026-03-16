use async_trait::async_trait;
use ctmint_core::error::Result;
use ctmint_core::vector::{SearchFilters, SearchResult, VectorMetadata};
use std::sync::{Arc, RwLock};

// ── Trait ────────────────────────────────────────────────────────────

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(
        &self,
        id: &str,
        vector: &[f32],
        metadata: VectorMetadata,
        content: Option<String>,
    ) -> Result<()>;

    async fn search(
        &self,
        vector: &[f32],
        filters: &SearchFilters,
        top_k: usize,
    ) -> Result<Vec<SearchResult>>;

    async fn delete(&self, id: &str) -> Result<()>;
}

// ── In-memory implementation (for testing and Cycle 0) ──────────────

struct StoredVector {
    id: String,
    vector: Vec<f32>,
    metadata: VectorMetadata,
    content: Option<String>,
}

#[derive(Default, Clone)]
pub struct InMemoryVectorStore {
    entries: Arc<RwLock<Vec<StoredVector>>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn upsert(
        &self,
        id: &str,
        vector: &[f32],
        metadata: VectorMetadata,
        content: Option<String>,
    ) -> Result<()> {
        let mut entries = self.entries.write().unwrap();
        entries.retain(|e| e.id != id);
        entries.push(StoredVector {
            id: id.to_string(),
            vector: vector.to_vec(),
            metadata,
            content,
        });
        Ok(())
    }

    async fn search(
        &self,
        vector: &[f32],
        filters: &SearchFilters,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        let entries = self.entries.read().unwrap();

        let mut scored: Vec<(f32, &StoredVector)> = entries
            .iter()
            .filter(|e| {
                if let Some(ref pid) = filters.project_id {
                    if e.metadata.project_id != *pid {
                        return false;
                    }
                }
                if let Some(ref sid) = filters.service_id {
                    if e.metadata.service_id.as_deref() != Some(sid.as_str()) {
                        return false;
                    }
                }
                if let Some(ref ct) = filters.chunk_type {
                    if e.metadata.chunk_type != *ct {
                        return false;
                    }
                }
                true
            })
            .map(|e| (cosine_similarity(vector, &e.vector), e))
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        let results = scored
            .into_iter()
            .map(|(score, e)| SearchResult {
                id: e.id.clone(),
                score,
                metadata: e.metadata.clone(),
                content: e.content.clone(),
            })
            .collect();

        Ok(results)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut entries = self.entries.write().unwrap();
        entries.retain(|e| e.id != id);
        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctmint_core::vector::ChunkType;

    fn make_metadata(project: &str, service: Option<&str>) -> VectorMetadata {
        VectorMetadata {
            project_id: project.to_string(),
            service_id: service.map(String::from),
            symbol_id: None,
            file_path: None,
            chunk_type: ChunkType::Code,
            line_start: None,
            line_end: None,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_search() {
        let store = InMemoryVectorStore::new();
        store
            .upsert("a", &[1.0, 0.0, 0.0], make_metadata("demo", Some("auth")), Some("fn login()".into()))
            .await
            .unwrap();
        store
            .upsert("b", &[0.0, 1.0, 0.0], make_metadata("demo", Some("pay")), Some("fn pay()".into()))
            .await
            .unwrap();

        let results = store
            .search(&[1.0, 0.0, 0.0], &SearchFilters::default(), 5)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "a");
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn test_search_with_filter() {
        let store = InMemoryVectorStore::new();
        store
            .upsert("a", &[1.0, 0.0], make_metadata("demo", Some("auth")), None)
            .await
            .unwrap();
        store
            .upsert("b", &[1.0, 0.0], make_metadata("demo", Some("pay")), None)
            .await
            .unwrap();

        let filters = SearchFilters {
            service_id: Some("auth".into()),
            ..Default::default()
        };
        let results = store.search(&[1.0, 0.0], &filters, 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");
    }
}
