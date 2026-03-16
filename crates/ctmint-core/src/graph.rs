use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Node types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Service,
    Module,
    Class,
    Function,
    Endpoint,
    Database,
    DatabaseTable,
    Column,
    Index,
    LogEvent,
    Trace,
    Span,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{self:?}"));
        write!(f, "{s}")
    }
}

// ── Edge types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeType {
    Contains,
    Calls,
    Implements,
    Reads,
    Writes,
    Imports,
    DependsOn,
    ProducesLog,
    HasTrace,
    HasSpan,
    BelongsTo,
    HasColumn,
    HasIndex,
    HasPrimaryKey,
    ForeignKey,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{self:?}"));
        write!(f, "{s}")
    }
}

// ── Direction ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Outgoing,
    Incoming,
}

// ── Node ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub project_id: String,
    pub attrs: HashMap<String, serde_json::Value>,
}

impl Node {
    pub fn new(id: impl Into<String>, node_type: NodeType, project_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type,
            project_id: project_id.into(),
            attrs: HashMap::new(),
        }
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.attrs.insert(key.into(), value.into());
        self
    }

    pub fn attr_str(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).and_then(|v| v.as_str())
    }
}

// ── Edge ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: EdgeType,
    pub project_id: String,
    pub attrs: HashMap<String, serde_json::Value>,
}

impl Edge {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        edge_type: EdgeType,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            target_id: target_id.into(),
            edge_type,
            project_id: project_id.into(),
            attrs: HashMap::new(),
        }
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.attrs.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_builder() {
        let node = Node::new("service:auth", NodeType::Service, "demo")
            .with_attr("name", "auth-service")
            .with_attr("language", "rust");

        assert_eq!(node.id, "service:auth");
        assert_eq!(node.node_type, NodeType::Service);
        assert_eq!(node.attr_str("name"), Some("auth-service"));
    }

    #[test]
    fn test_edge_builder() {
        let edge = Edge::new("service:auth", "service:payment", EdgeType::Calls, "demo");
        assert_eq!(edge.source_id, "service:auth");
        assert_eq!(edge.edge_type, EdgeType::Calls);
    }

    #[test]
    fn test_serde_roundtrip() {
        let node = Node::new("func:auth::login", NodeType::Function, "demo")
            .with_attr("name", "login_user");
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.node_type, node.node_type);
    }
}
