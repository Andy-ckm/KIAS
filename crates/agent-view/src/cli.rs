//! CLI argument definitions — kubectl-style agent management.
//!
//! ```text
//! kias-agent-view status [AGENT_ID]     # Agent status overview
//! kias-agent-view logs AGID [-f] [-n N] # Stream/display agent logs
//! kias-agent-view top [AGENT_ID]        # Resource monitoring
//! kias-agent-view get [TYPE] [ID]       # List/get resources
//! kias-agent-view completion SHELL      # Generate shell completions
//! ```

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "kias-agent-view",
    about = "AgentGuard Agent View CLI — kubectl-style agent management",
    version,
    long_about = "Inspect, monitor, and debug AgentGuard agents.\n\
                  Modelled after kubectl for a familiar operator experience."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output format
    #[arg(short, long, global = true, default_value = "table")]
    pub output: OutputFormat,

    /// API server URL (overrides config)
    #[arg(short, long, global = true)]
    pub server: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Show agent status overview (like `kubectl get pods`)
    Status {
        /// Agent ID to inspect (omit for all)
        agent_id: Option<String>,
        /// Show wide output with extra columns
        #[arg(long)]
        wide: bool,
    },

    /// Display or stream agent logs (like `kubectl logs`)
    Logs {
        /// Agent or session ID
        id: String,
        /// Follow log output (stream)
        #[arg(short = 'f', long)]
        follow: bool,
        /// Number of recent lines to show
        #[arg(short = 'n', long, default_value = "100")]
        tail: usize,
        /// Filter by log level
        #[arg(short = 'L', long)]
        level: Option<String>,
        /// Filter by component/module
        #[arg(short = 'c', long)]
        component: Option<String>,
    },

    /// Resource monitoring (like `kubectl top`)
    Top {
        /// Agent ID to monitor (omit for all)
        agent_id: Option<String>,
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
        /// Sort by column (cpu, memory, tokens)
        #[arg(long, default_value = "cpu")]
        sort: String,
    },

    /// Get/list resources (like `kubectl get`)
    Get {
        /// Resource type: agents, sessions, tasks, nodes
        #[arg(default_value = "agents")]
        resource: ResourceType,
        /// Specific resource ID
        id: Option<String>,
        /// Show labels
        #[arg(long)]
        show_labels: bool,
        /// Watch for changes
        #[arg(long)]
        watch: bool,
    },

    /// Describe a resource in detail (like `kubectl describe`)
    Describe {
        /// Resource type
        resource: ResourceType,
        /// Resource ID
        id: String,
    },

    /// Generate shell completions
    Completion {
        /// Shell type
        shell: ShellType,
    },

    /// Show cluster/node overview
    Cluster {
        /// Show node details
        #[arg(long)]
        nodes: bool,
    },
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable table (default)
    Table,
    /// JSON output
    Json,
    /// YAML output
    Yaml,
    /// Quiet mode — IDs only
    Quiet,
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum ResourceType {
    Agents,
    Sessions,
    Tasks,
    Nodes,
    Workflows,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}
