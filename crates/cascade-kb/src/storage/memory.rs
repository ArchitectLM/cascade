use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

use crate::data::{CoreError, Entity, EntityRef, Scope};
use crate::storage::{KnowledgeStore, QueryParams, StorageFactory};

/// In-memory storage implementation
pub struct MemoryStore {
    entities: Arc<RwLock<HashMap<EntityRef, Entity>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            entities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn matches_filters(entity: &Entity, filters: &HashMap<String, String>) -> bool {
        for (key, value) in filters {
            match key.as_str() {
                "type" => if entity.entity_ref.entity_type != *value { return false },
                "id" => if entity.entity_ref.entity_id != *value { return false },
                _ => if let Some(metadata_value) = entity.metadata.properties.get(key) {
                    if metadata_value != value { return false }
                } else {
                    return false
                }
            }
        }
        true
    }
}

#[async_trait]
impl KnowledgeStore for MemoryStore {
    async fn store_entity(&self, entity: Entity) -> Result<(), CoreError> {
        let mut entities = self.entities.write().await;
        entities.insert(entity.entity_ref.clone(), entity);
        Ok(())
    }

    async fn get_entity(&self, entity_ref: &EntityRef) -> Result<Option<Entity>, CoreError> {
        let entities = self.entities.read().await;
        Ok(entities.get(entity_ref).cloned())
    }

    async fn delete_entity(&self, entity_ref: &EntityRef) -> Result<(), CoreError> {
        let mut entities = self.entities.write().await;
        entities.remove(entity_ref);
        Ok(())
    }

    async fn list_entities(
        &self,
        entity_type: &str,
        scope: &Scope,
        params: &QueryParams,
    ) -> Result<Vec<Entity>, CoreError> {
        let entities = self.entities.read().await;
        
        let mut filtered: Vec<Entity> = entities
            .values()
            .filter(|e| e.entity_ref.entity_type == entity_type)
            .filter(|e| e.scope == *scope)
            .filter(|e| Self::matches_filters(e, &params.filters))
            .cloned()
            .collect();

        if let Some(sort_by) = &params.sort_by {
            filtered.sort_by(|a, b| {
                let a_val = a.metadata.properties.get(sort_by);
                let b_val = b.metadata.properties.get(sort_by);
                match (a_val, b_val) {
                    (Some(a), Some(b)) => {
                        if params.sort_order.as_deref() == Some("desc") {
                            b.cmp(a)
                        } else {
                            a.cmp(b)
                        }
                    }
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }

        if let Some(offset) = params.offset {
            filtered = filtered.into_iter().skip(offset).collect();
        }

        if let Some(limit) = params.limit {
            filtered = filtered.into_iter().take(limit).collect();
        }

        Ok(filtered)
    }

    async fn semantic_search(
        &self,
        query_text: &str,
        entity_type: Option<&str>,
        scope: &Scope,
        limit: usize,
    ) -> Result<Vec<Entity>, CoreError> {
        // Simple implementation that just does substring matching
        // In a real implementation, this would use embeddings and vector similarity
        let entities = self.entities.read().await;
        
        let mut matches: Vec<Entity> = entities
            .values()
            .filter(|e| {
                if let Some(typ) = entity_type {
                    if e.entity_ref.entity_type != typ {
                        return false;
                    }
                }
                if e.scope != *scope {
                    return false;
                }
                e.metadata.properties.values().any(|v| v.contains(query_text))
            })
            .take(limit)
            .cloned()
            .collect();

        matches.sort_by(|a, b| {
            let a_score = a.metadata.properties.values()
                .filter(|v| v.contains(query_text))
                .count();
            let b_score = b.metadata.properties.values()
                .filter(|v| v.contains(query_text))
                .count();
            b_score.cmp(&a_score)
        });

        Ok(matches)
    }

    async fn graph_query(
        &self,
        start_entity: &EntityRef,
        traversal_params: HashMap<String, String>,
    ) -> Result<Vec<Entity>, CoreError> {
        // Simple implementation that returns connected entities
        // In a real implementation, this would do proper graph traversal
        let entities = self.entities.read().await;
        
        if let Some(start) = entities.get(start_entity) {
            let mut results = Vec::new();
            
            // Get all entities that are referenced by the start entity
            for ref_id in &start.metadata.references {
                if let Some(referenced) = entities.get(ref_id) {
                    if traversal_params.iter().all(|(k, v)| {
                        referenced.metadata.properties.get(k) == Some(v)
                    }) {
                        results.push(referenced.clone());
                    }
                }
            }
            
            Ok(results)
        } else {
            Ok(Vec::new())
        }
    }
}

/// Memory storage factory
pub struct MemoryStorageFactory;

#[async_trait]
impl StorageFactory for MemoryStorageFactory {
    async fn create_storage(&self) -> Result<Box<dyn KnowledgeStore>, CoreError> {
        Ok(Box::new(MemoryStore::new()))
    }
} 