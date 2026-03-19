use async_trait::async_trait;
use ctmint_core::error::{CtmintError, Result};
use ctmint_core::graph::{Direction, Edge, EdgeType, Node, NodeType};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_iso8601() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            let millis = d.subsec_millis();
            format!("{}.{:03}Z", secs, millis)
        })
        .unwrap_or_else(|_| "0".to_string())
}

fn node_type_to_str(t: &NodeType) -> String {
    serde_json::to_value(t).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| format!("{t:?}"))
}

fn edge_type_to_str(t: &EdgeType) -> String {
    serde_json::to_value(t).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| format!("{t:?}"))
}

fn str_to_node_type(s: &str) -> NodeType {
    serde_json::from_str(&format!("\"{s}\"")).unwrap_or(NodeType::Service)
}

fn str_to_edge_type(s: &str) -> EdgeType {
    serde_json::from_str(&format!("\"{s}\"")).unwrap_or(EdgeType::Calls)
}

fn map_rusqlite(e: rusqlite::Error) -> CtmintError {
    CtmintError::Rusqlite(e.to_string())
}

#[derive(Clone)]
pub struct SqliteGraphStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteGraphStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CtmintError::Storage(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| CtmintError::Storage(e.to_string()))?;
        let store = Self { conn: Arc::new(Mutex::new(conn)) };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                project_id TEXT NOT NULL,
                attrs TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_type_project ON nodes(type, project_id);

            CREATE TABLE IF NOT EXISTS edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                type TEXT NOT NULL,
                project_id TEXT NOT NULL,
                attrs TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (source_id, target_id, type)
            );
            CREATE INDEX IF NOT EXISTS idx_edges_source_type ON edges(source_id, type);
            CREATE INDEX IF NOT EXISTS idx_edges_target_type ON edges(target_id, type);
            CREATE INDEX IF NOT EXISTS idx_edges_project ON edges(project_id);
            "#,
        )
        .map_err(|e| CtmintError::Storage(e.to_string()))?;
        Ok(())
    }

}

#[async_trait]
impl super::graph::GraphStore for SqliteGraphStore {
    async fn upsert_node(&self, node: Node) -> Result<()> {
        let node = node;
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let now = now_iso8601();
            let attrs = serde_json::to_string(&node.attrs).unwrap_or_else(|_| "{}".to_string());
            conn.execute(
                r#"
                INSERT INTO nodes (id, type, project_id, attrs, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    type = excluded.type,
                    project_id = excluded.project_id,
                    attrs = excluded.attrs,
                    updated_at = excluded.updated_at
                "#,
                params![
                    node.id,
                    node_type_to_str(&node.node_type),
                    node.project_id,
                    attrs,
                    now,
                    now,
                ],
            )
            .map_err(|e| CtmintError::Storage(e.to_string()))?;
            Ok::<(), CtmintError>(())
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn upsert_edge(&self, edge: Edge) -> Result<()> {
        let edge = edge;
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let now = now_iso8601();
            let attrs = serde_json::to_string(&edge.attrs).unwrap_or_else(|_| "{}".to_string());
            conn.execute(
                r#"
                INSERT INTO edges (source_id, target_id, type, project_id, attrs, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(source_id, target_id, type) DO UPDATE SET
                    project_id = excluded.project_id,
                    attrs = excluded.attrs,
                    updated_at = excluded.updated_at
                "#,
                params![
                    edge.source_id,
                    edge.target_id,
                    edge_type_to_str(&edge.edge_type),
                    edge.project_id,
                    attrs,
                    now,
                    now,
                ],
            )
            .map_err(|e| CtmintError::Storage(e.to_string()))?;
            Ok::<(), CtmintError>(())
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn get_node(&self, id: &str) -> Result<Option<Node>> {
        let id = id.to_string();
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut stmt = conn
                .prepare("SELECT id, type, project_id, attrs FROM nodes WHERE id = ?1")
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut rows = stmt
                .query(params![id])
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            if let Some(row) = rows.next().map_err(map_rusqlite)? {
                let id: String = row.get(0).map_err(map_rusqlite)?;
                let type_str: String = row.get(1).map_err(map_rusqlite)?;
                let project_id: String = row.get(2).map_err(map_rusqlite)?;
                let attrs_str: String = row.get(3).map_err(map_rusqlite)?;
                let attrs: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&attrs_str).unwrap_or_default();
                let node = Node {
                    id,
                    node_type: str_to_node_type(&type_str),
                    project_id,
                    attrs,
                };
                return Ok(Some(node));
            }
            Ok(None)
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn get_neighbors(
        &self,
        node_id: &str,
        edge_type: Option<EdgeType>,
        direction: Direction,
    ) -> Result<Vec<Node>> {
        let node_id = node_id.to_string();
        let edge_type = edge_type;
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let (col, id_col) = match direction {
                Direction::Outgoing => ("source_id", "target_id"),
                Direction::Incoming => ("target_id", "source_id"),
            };
            let sql = if edge_type.is_some() {
                format!(
                    "SELECT n.id, n.type, n.project_id, n.attrs FROM edges e
                     JOIN nodes n ON n.id = e.{id_col}
                     WHERE e.{col} = ?1 AND e.type = ?2"
                )
            } else {
                format!(
                    "SELECT n.id, n.type, n.project_id, n.attrs FROM edges e
                     JOIN nodes n ON n.id = e.{id_col}
                     WHERE e.{col} = ?1"
                )
            };
            let mut stmt = conn.prepare(&sql).map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut rows = if let Some(et) = &edge_type {
                stmt.query(params![node_id, edge_type_to_str(et)])
            } else {
                stmt.query(params![node_id])
            }
            .map_err(map_rusqlite)?;
            let mut nodes = Vec::new();
            while let Some(row) = rows.next().map_err(map_rusqlite)? {
                let id: String = row.get(0).map_err(map_rusqlite)?;
                let type_str: String = row.get(1).map_err(map_rusqlite)?;
                let project_id: String = row.get(2).map_err(map_rusqlite)?;
                let attrs_str: String = row.get(3).map_err(map_rusqlite)?;
                let attrs: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&attrs_str).unwrap_or_default();
                nodes.push(Node {
                    id,
                    node_type: str_to_node_type(&type_str),
                    project_id,
                    attrs,
                });
            }
            Ok(nodes)
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn batch_commit(&self, nodes: Vec<Node>, edges: Vec<Edge>) -> Result<()> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let tx = conn.unchecked_transaction().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let now = now_iso8601();
            for node in nodes {
                let attrs = serde_json::to_string(&node.attrs).unwrap_or_else(|_| "{}".to_string());
                tx.execute(
                    r#"
                    INSERT INTO nodes (id, type, project_id, attrs, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ON CONFLICT(id) DO UPDATE SET
                        type = excluded.type, project_id = excluded.project_id,
                        attrs = excluded.attrs, updated_at = excluded.updated_at
                    "#,
                    params![node.id, node_type_to_str(&node.node_type), node.project_id, attrs, now, now],
                )
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            }
            for edge in edges {
                let attrs = serde_json::to_string(&edge.attrs).unwrap_or_else(|_| "{}".to_string());
                tx.execute(
                    r#"
                    INSERT INTO edges (source_id, target_id, type, project_id, attrs, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ON CONFLICT(source_id, target_id, type) DO UPDATE SET
                        project_id = excluded.project_id, attrs = excluded.attrs, updated_at = excluded.updated_at
                    "#,
                    params![
                        edge.source_id,
                        edge.target_id,
                        edge_type_to_str(&edge.edge_type),
                        edge.project_id,
                        attrs,
                        now,
                        now,
                    ],
                )
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            }
            tx.commit().map_err(|e| CtmintError::Storage(e.to_string()))?;
            Ok::<(), CtmintError>(())
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn delete_node(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            conn.execute("DELETE FROM edges WHERE source_id = ?1 OR target_id = ?1", params![id])
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            conn.execute("DELETE FROM nodes WHERE id = ?1", params![id])
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            Ok::<(), CtmintError>(())
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn get_nodes_by_type(
        &self,
        node_type: NodeType,
        project_id: &str,
    ) -> Result<Vec<Node>> {
        let type_str = node_type_to_str(&node_type);
        let project_id = project_id.to_string();
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut stmt = conn
                .prepare("SELECT id, type, project_id, attrs FROM nodes WHERE type = ?1 AND project_id = ?2")
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut rows = stmt
                .query(params![type_str, project_id])
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut nodes = Vec::new();
            while let Some(row) = rows.next().map_err(map_rusqlite)? {
                let id: String = row.get(0).map_err(map_rusqlite)?;
                let type_str: String = row.get(1).map_err(map_rusqlite)?;
                let project_id: String = row.get(2).map_err(map_rusqlite)?;
                let attrs_str: String = row.get(3).map_err(map_rusqlite)?;
                let attrs: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&attrs_str).unwrap_or_default();
                nodes.push(Node {
                    id,
                    node_type: str_to_node_type(&type_str),
                    project_id,
                    attrs,
                });
            }
            Ok(nodes)
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }

    async fn get_edges_by_type(
        &self,
        edge_type: EdgeType,
        project_id: &str,
    ) -> Result<Vec<Edge>> {
        let type_str = edge_type_to_str(&edge_type);
        let project_id = project_id.to_string();
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = store.conn.lock().map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut stmt = conn
                .prepare("SELECT source_id, target_id, type, project_id, attrs FROM edges WHERE type = ?1 AND project_id = ?2")
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut rows = stmt
                .query(params![type_str, project_id])
                .map_err(|e| CtmintError::Storage(e.to_string()))?;
            let mut edges = Vec::new();
            while let Some(row) = rows.next().map_err(map_rusqlite)? {
                let source_id: String = row.get(0).map_err(map_rusqlite)?;
                let target_id: String = row.get(1).map_err(map_rusqlite)?;
                let type_str: String = row.get(2).map_err(map_rusqlite)?;
                let project_id: String = row.get(3).map_err(map_rusqlite)?;
                let attrs_str: String = row.get(4).map_err(map_rusqlite)?;
                let attrs: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&attrs_str).unwrap_or_default();
                edges.push(Edge {
                    source_id,
                    target_id,
                    edge_type: str_to_edge_type(&type_str),
                    project_id,
                    attrs,
                });
            }
            Ok(edges)
        })
        .await
        .map_err(|e| CtmintError::Storage(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphStore;
    use ctmint_core::graph::Direction;

    #[tokio::test]
    async fn test_sqlite_upsert_and_get() {
        let store = SqliteGraphStore::open(":memory:").unwrap();
        let node = Node::new("service:auth", NodeType::Service, "demo")
            .with_attr("name", "auth-service");
        store.upsert_node(node).await.unwrap();
        let got = store.get_node("service:auth").await.unwrap().unwrap();
        assert_eq!(got.id, "service:auth");
        assert_eq!(got.attr_str("name"), Some("auth-service"));
    }

    #[tokio::test]
    async fn test_sqlite_neighbors() {
        let store = SqliteGraphStore::open(":memory:").unwrap();
        let auth = Node::new("service:auth", NodeType::Service, "demo");
        let payment = Node::new("service:payment", NodeType::Service, "demo");
        let edge = Edge::new("service:auth", "service:payment", EdgeType::Calls, "demo");
        store.batch_commit(vec![auth, payment], vec![edge]).await.unwrap();

        let out = store
            .get_neighbors("service:auth", Some(EdgeType::Calls), Direction::Outgoing)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "service:payment");
    }

    #[tokio::test]
    async fn test_sqlite_get_architecture_map() {
        let store = SqliteGraphStore::open(":memory:").unwrap();
        let auth = Node::new("service:auth", NodeType::Service, "demo")
            .with_attr("name", "auth-service");
        let payment = Node::new("service:payment", NodeType::Service, "demo")
            .with_attr("name", "payment-service");
        let edge = Edge::new("service:auth", "service:payment", EdgeType::Calls, "demo");
        store
            .batch_commit(vec![auth, payment], vec![edge])
            .await
            .unwrap();

        let map = store.get_architecture_map("demo").await.unwrap();
        assert_eq!(map.nodes.len(), 2);
        assert_eq!(map.edges.len(), 1);
        assert_eq!(map.edges[0].0, "service:auth");
        assert_eq!(map.edges[0].1, "service:payment");
    }

    #[tokio::test]
    async fn test_sqlite_get_service_graph() {
        let store = SqliteGraphStore::open(":memory:").unwrap();
        let auth = Node::new("service:auth-service", NodeType::Service, "demo")
            .with_attr("name", "auth-service");
        let module = Node::new("module:auth::api", NodeType::Module, "demo");
        let edge1 = Edge::new(
            "service:auth-service",
            "module:auth::api",
            EdgeType::Contains,
            "demo",
        );
        store
            .batch_commit(vec![auth, module], vec![edge1])
            .await
            .unwrap();

        let subgraph = store.get_service_graph("auth-service", "demo").await.unwrap();
        assert_eq!(subgraph.nodes.len(), 2);
        assert_eq!(subgraph.edges.len(), 1);
    }
}
