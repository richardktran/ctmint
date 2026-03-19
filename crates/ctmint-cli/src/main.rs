mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ctmint",
    about = "ContextMint — AI-native system debugger",
    version,
    long_about = "Unify code, runtime (logs/traces), and data (DB schema) into a System Knowledge Graph.\nLet AI agents answer \"Why is X failing?\" by traversing the graph."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run AI-guided onboarding: detect source code, logs, DB, tracing and generate ctmint.yaml
    Init {
        /// Path to source code repository (if omitted, you'll be prompted)
        #[arg(long)]
        path: Option<String>,

        /// Output path for the project manifest (if omitted, defaults to <project>.yaml in repo root)
        #[arg(long)]
        output: Option<String>,

        /// Skip AI model and use guided question flow only
        #[arg(long)]
        no_ai: bool,

        /// Overwrite existing manifest without asking
        #[arg(long)]
        force: bool,

        /// Generate a sample manifest without prompting (for CI or quick test)
        #[arg(long)]
        demo: bool,
    },

    /// Download the onboarding AI model (~484 MB) for use with `ctmint init`
    DownloadModel,

    /// Index the codebase: parse source, build symbol graph, populate SKG
    Index {
        /// Path to project manifest
        #[arg(long, default_value = "ctmint.yaml")]
        project: String,
    },

    /// Graph operations: query the System Knowledge Graph
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },

    /// Start the MCP server (stdio)
    Serve,
}

#[derive(Subcommand)]
enum GraphAction {
    /// List all services in the graph
    ListServices {
        /// Path to project manifest
        #[arg(long, default_value = "ctmint.yaml")]
        project: String,
    },

    /// Query the subgraph of a specific service
    Query {
        /// Service name to query
        #[arg(long)]
        service: String,

        /// Path to project manifest
        #[arg(long, default_value = "ctmint.yaml")]
        project: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            path,
            output,
            no_ai,
            force,
            demo,
        } => {
            commands::init::run(path.as_deref(), output.as_deref(), no_ai, force, demo).await;
        }
        Commands::DownloadModel => {
            commands::download_model::run().await;
        }
        Commands::Index { project } => {
            commands::index::run(&project).await;
        }
        Commands::Graph { action } => match action {
            GraphAction::ListServices { project } => {
                commands::graph::list_services(&project).await;
            }
            GraphAction::Query { service, project } => {
                commands::graph::query_service(&service, &project).await;
            }
        },
        Commands::Serve => {
            commands::serve::run().await;
        }
    }
}
