use std::collections::HashMap;
use crate::data::{Entity, EntityRef, Metadata, Scope};
use crate::storage::{KnowledgeStore, QueryParams, MemoryStore};

#[tokio::test]
async fn test_basic_crud_operations() {
    let store = MemoryStore::new();
    
    // Create test entity
    let entity_ref = EntityRef {
        entity_type: "test".to_string(),
        entity_id: "123".to_string(),
    };
    
    let mut properties = HashMap::new();
    properties.insert("name".to_string(), "Test Entity".to_string());
    
    let entity = Entity {
        entity_ref: entity_ref.clone(),
        scope: Scope::Global,
        metadata: Metadata {
            properties,
            references: vec![],
        },
    };

    // Test store
    store.store_entity(entity.clone()).await.unwrap();

    // Test get
    let retrieved = store.get_entity(&entity_ref).await.unwrap().unwrap();
    assert_eq!(retrieved.entity_ref, entity_ref);
    assert_eq!(retrieved.metadata.properties.get("name").unwrap(), "Test Entity");

    // Test list
    let params = QueryParams {
        filters: HashMap::new(),
        sort_by: None,
        sort_order: None,
        offset: None,
        limit: None,
    };
    let list = store.list_entities("test", &Scope::Global, &params).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].entity_ref, entity_ref);

    // Test delete
    store.delete_entity(&entity_ref).await.unwrap();
    let not_found = store.get_entity(&entity_ref).await.unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_filtering_and_pagination() {
    let store = MemoryStore::new();
    
    // Create multiple test entities
    for i in 0..10 {
        let entity_ref = EntityRef {
            entity_type: "test".to_string(),
            entity_id: i.to_string(),
        };
        
        let mut properties = HashMap::new();
        properties.insert("name".to_string(), format!("Entity {}", i));
        properties.insert("category".to_string(), if i % 2 == 0 { "even".to_string() } else { "odd".to_string() });
        
        let entity = Entity {
            entity_ref: entity_ref.clone(),
            scope: Scope::Global,
            metadata: Metadata {
                properties,
                references: vec![],
            },
        };
        
        store.store_entity(entity).await.unwrap();
    }

    // Test filtering
    let mut filters = HashMap::new();
    filters.insert("category".to_string(), "even".to_string());
    
    let params = QueryParams {
        filters,
        sort_by: Some("name".to_string()),
        sort_order: Some("asc".to_string()),
        offset: None,
        limit: None,
    };
    
    let filtered = store.list_entities("test", &Scope::Global, &params).await.unwrap();
    assert_eq!(filtered.len(), 5);
    assert!(filtered.iter().all(|e| e.metadata.properties.get("category").unwrap() == "even"));

    // Test pagination
    let params = QueryParams {
        filters: HashMap::new(),
        sort_by: Some("name".to_string()),
        sort_order: Some("asc".to_string()),
        offset: Some(5),
        limit: Some(3),
    };
    
    let paginated = store.list_entities("test", &Scope::Global, &params).await.unwrap();
    assert_eq!(paginated.len(), 3);
}

#[tokio::test]
async fn test_semantic_search() {
    let store = MemoryStore::new();
    
    // Create test entities with varying degrees of match
    let entities = vec![
        ("1", "This is a test document about rust programming"),
        ("2", "Another document mentioning rust briefly"),
        ("3", "Document about python programming"),
        ("4", "Rust is awesome for systems programming"),
    ];
    
    for (id, content) in entities {
        let entity_ref = EntityRef {
            entity_type: "document".to_string(),
            entity_id: id.to_string(),
        };
        
        let mut properties = HashMap::new();
        properties.insert("content".to_string(), content.to_string());
        
        let entity = Entity {
            entity_ref: entity_ref.clone(),
            scope: Scope::Global,
            metadata: Metadata {
                properties,
                references: vec![],
            },
        };
        
        store.store_entity(entity).await.unwrap();
    }

    // Test semantic search
    let results = store.semantic_search("rust programming", Some("document"), &Scope::Global, 10).await.unwrap();
    assert!(!results.is_empty());
    
    // First result should be the most relevant
    let first_content = results[0].metadata.properties.get("content").unwrap();
    assert!(first_content.contains("rust") && first_content.contains("programming"));
}

#[tokio::test]
async fn test_graph_query() {
    let store = MemoryStore::new();
    
    // Create a network of related entities
    let root_ref = EntityRef {
        entity_type: "person".to_string(),
        entity_id: "root".to_string(),
    };
    
    let child1_ref = EntityRef {
        entity_type: "person".to_string(),
        entity_id: "child1".to_string(),
    };
    
    let child2_ref = EntityRef {
        entity_type: "person".to_string(),
        entity_id: "child2".to_string(),
    };
    
    // Root entity with references to children
    let root_entity = Entity {
        entity_ref: root_ref.clone(),
        scope: Scope::Global,
        metadata: Metadata {
            properties: {
                let mut props = HashMap::new();
                props.insert("name".to_string(), "Root".to_string());
                props.insert("role".to_string(), "parent".to_string());
                props
            },
            references: vec![child1_ref.clone(), child2_ref.clone()],
        },
    };
    
    // Child entities
    let child1_entity = Entity {
        entity_ref: child1_ref.clone(),
        scope: Scope::Global,
        metadata: Metadata {
            properties: {
                let mut props = HashMap::new();
                props.insert("name".to_string(), "Child 1".to_string());
                props.insert("role".to_string(), "child".to_string());
                props
            },
            references: vec![],
        },
    };
    
    let child2_entity = Entity {
        entity_ref: child2_ref.clone(),
        scope: Scope::Global,
        metadata: Metadata {
            properties: {
                let mut props = HashMap::new();
                props.insert("name".to_string(), "Child 2".to_string());
                props.insert("role".to_string(), "child".to_string());
                props
            },
            references: vec![],
        },
    };
    
    // Store all entities
    store.store_entity(root_entity).await.unwrap();
    store.store_entity(child1_entity).await.unwrap();
    store.store_entity(child2_entity).await.unwrap();

    // Test graph query
    let mut params = HashMap::new();
    params.insert("role".to_string(), "child".to_string());
    
    let results = store.graph_query(&root_ref, params).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|e| e.metadata.properties.get("role").unwrap() == "child"));
} 