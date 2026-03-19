use ctmint_config::{GlobalConfig, ProjectManifest};
use ctmint_core::graph::{Edge, EdgeType, Node, NodeType};
use ctmint_storage::graph::GraphStore;
use ctmint_storage::SqliteGraphStore;
use std::path::Path;

fn open_store() -> SqliteGraphStore {
    let config = GlobalConfig::resolve();
    std::fs::create_dir_all(&config.data_dir).expect("create data_dir");
    SqliteGraphStore::open(config.graph_db_path()).expect("open graph store")
}

pub async fn load_sample(project_id: &str) {
    let store = open_store();

    let nodes = vec![
        Node::new("service:auth-service", NodeType::Service, project_id)
            .with_attr("name", "auth-service")
            .with_attr("repo_path", "./services/auth")
            .with_attr("language", "rust"),
        Node::new("service:user-service", NodeType::Service, project_id)
            .with_attr("name", "user-service")
            .with_attr("repo_path", "./services/user")
            .with_attr("language", "rust"),
        Node::new("service:payment-service", NodeType::Service, project_id)
            .with_attr("name", "payment-service")
            .with_attr("repo_path", "./services/payment")
            .with_attr("language", "rust"),
        Node::new("module:auth::api", NodeType::Module, project_id)
            .with_attr("name", "auth.api")
            .with_attr("service_id", "service:auth-service"),
        Node::new("func:auth::login", NodeType::Function, project_id)
            .with_attr("name", "login")
            .with_attr("file_path", "services/auth/src/api.rs")
            .with_attr("line_start", 10)
            .with_attr("line_end", 25)
            .with_attr("service_id", "service:auth-service"),
        Node::new("func:user::get_user", NodeType::Function, project_id)
            .with_attr("name", "get_user")
            .with_attr("file_path", "services/user/src/handlers.rs")
            .with_attr("line_start", 5)
            .with_attr("line_end", 20)
            .with_attr("service_id", "service:user-service"),
    ];

    let edges = vec![
        Edge::new(
            "service:auth-service",
            "module:auth::api",
            EdgeType::Contains,
            project_id,
        ),
        Edge::new(
            "module:auth::api",
            "func:auth::login",
            EdgeType::Contains,
            project_id,
        ),
        Edge::new(
            "service:auth-service",
            "service:user-service",
            EdgeType::Calls,
            project_id,
        ),
        Edge::new(
            "service:auth-service",
            "service:payment-service",
            EdgeType::Calls,
            project_id,
        ),
        Edge::new(
            "service:user-service",
            "func:user::get_user",
            EdgeType::Contains,
            project_id,
        ),
    ];

    if let Err(e) = store.batch_commit(nodes, edges).await {
        eprintln!("Error loading sample graph: {e}");
        std::process::exit(1);
    }

    println!("Sample graph loaded for project '{project_id}'.");
    println!("  Services: auth-service, user-service, payment-service");
    println!("  Run: ctmint graph query --service auth-service --project {project_id}");
}

fn resolve_project_id(project: &str) -> String {
    let path = Path::new(project);
    if path.extension().map_or(false, |e| e == "yaml" || e == "yml") && path.is_file() {
        match ProjectManifest::load(path) {
            Ok(m) => m.project,
            Err(e) => {
                eprintln!("Error loading manifest: {e}");
                std::process::exit(1);
            }
        }
    } else {
        project.to_string()
    }
}

pub async fn list_services(project: &str) {
    let project_id = resolve_project_id(project);

    let store = open_store();
    match store.get_nodes_by_type(NodeType::Service, &project_id).await {
        Ok(services) => {
            println!("ContextMint — Services (from graph)");
            println!("====================================");
            if services.is_empty() {
                println!("  No services in graph. Run `ctmint graph load-sample --project {project_id}` first.");
            } else {
                for svc in &services {
                    let name = svc.attr_str("name").unwrap_or(&svc.id);
                    let lang = svc.attr_str("language").unwrap_or("?");
                    let path = svc.attr_str("repo_path").unwrap_or("?");
                    println!("  {name} ({lang}) — {path}");
                }
            }
        }
        Err(e) => {
            eprintln!("Error querying graph: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn query_service(service: &str, project: &str) {
    let project_id = resolve_project_id(project);

    let store = open_store();
    match store.get_service_graph(service, &project_id).await {
        Ok(subgraph) => {
            println!("ContextMint — Graph Query");
            println!("=========================");
            println!("Service: {service}");
            println!();
            println!("Nodes ({}):", subgraph.nodes.len());
            for n in &subgraph.nodes {
                let name = n.attr_str("name").unwrap_or(&n.id);
                println!("  - {} ({}) id={}", name, n.node_type, n.id);
            }
            println!();
            println!("Edges ({}):", subgraph.edges.len());
            for e in &subgraph.edges {
                println!("  {} -[{}]-> {}", e.source_id, e.edge_type, e.target_id);
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
