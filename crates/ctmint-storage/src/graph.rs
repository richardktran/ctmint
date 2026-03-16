use async_trait::async_trait;
use ctmint_core::error::Result;
use ctmint_core::graph::{Direction, Edge, EdgeType, Node, NodeType};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// ── Trait ────────────────────────────────────────────────────────────

#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn upsert_node(&self, node: Node) -> Result<()>;

    async fn upsert_edge(&self, edge: Edge) -> Result<()>;

    async fn get_node(&self, id: &str) -> Result<Option<Node>>;

    async fn get_neighbors(
        &self,
        node_id: &str,
        edge_type: Option<EdgeType>,
        direction: Direction,
    ) -> Result<Vec<Node>>;

    async fn batch_commit(&self, nodes: Vec<Node>, edges: Vec<Edge>) -> Result<()>;

    async fn delete_node(&self, id: &str) -> Result<()>;

    async fn get_nodes_by_type(
        &self,
        node_type: NodeType,
        project_id: &str,
    ) -> Result<Vec<Node>>;

    async fn get_edges_by_type(
        &self,
        edge_type: EdgeType,
        project_id: &str,
    ) -> Result<Vec<Edge>>;
}

// ── In-memory implementation (for testing and Cycle 0) ──────────────

#[derive(Default, Clone)]
pub struct InMemoryGraphStore {
    nodes: Arc<RwLock<HashMap<String, Node>>>,
    edges: Arc<RwLock<Vec<Edge>>>,
}

impl InMemoryGraphStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl GraphStore for InMemoryGraphStore {
    async fn upsert_node(&self, node: Node) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id.clone(), node);
        Ok(())
    }

    async fn upsert_edge(&self, edge: Edge) -> Result<()> {
        let mut edges = self.edges.write().unwrap();
        edges.retain(|e| {
            !(e.source_id == edge.source_id
                && e.target_id == edge.target_id
                && e.edge_type == edge.edge_type)
        });
        edges.push(edge);
        Ok(())
    }

    async fn get_node(&self, id: &str) -> Result<Option<Node>> {
        let nodes = self.nodes.read().unwrap();
        Ok(nodes.get(id).cloned())
    }

    async fn get_neighbors(
        &self,
        node_id: &str,
        edge_type: Option<EdgeType>,
        direction: Direction,
    ) -> Result<Vec<Node>> {
        let edges = self.edges.read().unwrap();
        let nodes = self.nodes.read().unwrap();

        let neighbor_ids: Vec<String> = edges
            .iter()
            .filter(|e| {
                let type_match = edge_type
                    .as_ref()
                    .map_or(true, |et| e.edge_type == *et);
                let dir_match = match direction {
                    Direction::Outgoing => e.source_id == node_id,
                    Direction::Incoming => e.target_id == node_id,
                };
                type_match && dir_match
            })
            .map(|e| match direction {
                Direction::Outgoing => e.target_id.clone(),
                Direction::Incoming => e.source_id.clone(),
            })
            .collect();

        let result = neighbor_ids
            .iter()
            .filter_map(|id| nodes.get(id).cloned())
            .collect();

        Ok(result)
    }

    async fn batch_commit(&self, nodes: Vec<Node>, edges: Vec<Edge>) -> Result<()> {
        for node in nodes {
            self.upsert_node(node).await?;
        }
        for edge in edges {
            self.upsert_edge(edge).await?;
        }
        Ok(())
    }

    async fn delete_node(&self, id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        nodes.remove(id);
        let mut edges = self.edges.write().unwrap();
        edges.retain(|e| e.source_id != id && e.target_id != id);
        Ok(())
    }

    async fn get_nodes_by_type(
        &self,
        node_type: NodeType,
        project_id: &str,
    ) -> Result<Vec<Node>> {
        let nodes = self.nodes.read().unwrap();
        let result = nodes
            .values()
            .filter(|n| n.node_type == node_type && n.project_id == project_id)
            .cloned()
            .collect();
        Ok(result)
    }

    async fn get_edges_by_type(
        &self,
        edge_type: EdgeType,
        project_id: &str,
    ) -> Result<Vec<Edge>> {
        let edges = self.edges.read().unwrap();
        let result = edges
            .iter()
            .filter(|e| e.edge_type == edge_type && e.project_id == project_id)
            .cloned()
            .collect();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_upsert_and_get() {
        let store = InMemoryGraphStore::new();
        let node = Node::new("service:auth", NodeType::Service, "demo")
            .with_attr("name", "auth-service");

        store.upsert_node(node.clone()).await.unwrap();
        let got = store.get_node("service:auth").await.unwrap().unwrap();
        assert_eq!(got.id, "service:auth");
        assert_eq!(got.attr_str("name"), Some("auth-service"));
    }

    #[tokio::test]
    async fn test_neighbors() {
        let store = InMemoryGraphStore::new();
        let auth = Node::new("service:auth", NodeType::Service, "demo");
        let payment = Node::new("service:payment", NodeType::Service, "demo");
        let edge = Edge::new("service:auth", "service:payment", EdgeType::Calls, "demo");

        store
            .batch_commit(vec![auth, payment], vec![edge])
            .await
            .unwrap();

        let out = store
            .get_neighbors("service:auth", Some(EdgeType::Calls), Direction::Outgoing)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "service:payment");

        let inc = store
            .get_neighbors("service:payment", Some(EdgeType::Calls), Direction::Incoming)
            .await
            .unwrap();
        assert_eq!(inc.len(), 1);
        assert_eq!(inc[0].id, "service:auth");
    }

    #[tokio::test]
    async fn test_delete_node_cascades_edges() {
        let store = InMemoryGraphStore::new();
        let a = Node::new("a", NodeType::Service, "demo");
        let b = Node::new("b", NodeType::Service, "demo");
        let edge = Edge::new("a", "b", EdgeType::Calls, "demo");
        store.batch_commit(vec![a, b], vec![edge]).await.unwrap();

        store.delete_node("a").await.unwrap();
        assert!(store.get_node("a").await.unwrap().is_none());

        let neighbors = store
            .get_neighbors("b", None, Direction::Incoming)
            .await
            .unwrap();
        assert!(neighbors.is_empty());
    }
}
