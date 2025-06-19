use std::sync::Arc;

use libruskel::Ruskel;

#[derive(Debug)]
pub struct RuskelAnalyzer;

impl Default for RuskelAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RuskelAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate_skeleton(
        &mut self,
        target: &str,
        features: &[String],
        all_features: bool,
        no_default_features: bool,
        private: bool,
    ) -> tenx_mcp::Result<String> {
        let ruskel = Ruskel::new(target);

        // Apply feature flags if provided
        let mut ruskel = if no_default_features {
            ruskel.with_no_default_features(true)
        } else {
            ruskel
        };

        if all_features {
            ruskel = ruskel.with_all_features(true);
        } else if !features.is_empty() {
            ruskel = ruskel.with_features(features.to_vec());
        }

        // Generate the skeleton
        let skeleton = ruskel
            .render(private, false, false)
            .map_err(|e| tenx_mcp::Error::InternalError(format!("Ruskel error: {e}")))?;
        Ok(skeleton)
    }
}
use tenx_mcp::{
    Connection, Result, Server,
    schema::{
        CallToolResult, InitializeResult, ListToolsResult, ServerCapabilities, Tool,
        ToolInputSchema,
    },
};
use tracing::error;

pub struct RuskelConnection {
    ruskel: Arc<Ruskel>,
}

impl RuskelConnection {
    pub fn new(ruskel: Arc<Ruskel>) -> Self {
        Self { ruskel }
    }

    fn tool_metadata(&self) -> Tool {
        let properties = [
            ("target", serde_json::json!({
                "type": "string",
                "description": "Crate, module path, or filesystem path (optionally with @<semver>) whose API skeleton should be produced."
            })),
            ("private", serde_json::json!({
                "type": "boolean",
                "description": "Include non‑public (private / crate‑private) items.",
                "default": false
            })),
            ("no_default_features", serde_json::json!({
                "type": "boolean",
                "description": "Disable the crate's default Cargo features.",
                "default": false
            })),
            ("all_features", serde_json::json!({
                "type": "boolean",
                "description": "Enable every optional Cargo feature.",
                "default": false
            })),
            ("features", serde_json::json!({
                "type": "array",
                "items": {"type": "string"},
                "description": "Exact list of Cargo features to enable (ignored if all_features=true).",
                "default": []
            }))
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect();

        Tool::new(
            "ruskel",
            ToolInputSchema::default()
                .with_properties(properties)
                .with_required("target"),
        )
        .with_description(include_str!("ruskel-description.txt"))
    }

    async fn execute_tool(&self, arguments: Option<serde_json::Value>) -> CallToolResult {
        let args = arguments.unwrap_or_default();

        let tool_params: RuskelSkeletonTool = match serde_json::from_value(args) {
            Ok(params) => params,
            Err(e) => {
                return CallToolResult::new()
                    .with_text_content(format!("Invalid parameters for ruskel tool: {e}"))
                    .is_error(true);
            }
        };

        match self.ruskel.render(tool_params.private, false, false) {
            Ok(output) => CallToolResult::new().with_text_content(output),
            Err(e) => {
                error!("Failed to generate skeleton: {}", e);
                CallToolResult::new()
                    .with_text_content(format!(
                        "Failed to generate skeleton for '{}': {}",
                        tool_params.target, e
                    ))
                    .is_error(true)
            }
        }
    }
}

#[async_trait::async_trait]
impl Connection for RuskelConnection {
    async fn initialize(
        &mut self,
        _protocol_version: String,
        _capabilities: tenx_mcp::ClientCapabilities,
        _client_info: tenx_mcp::Implementation,
    ) -> Result<InitializeResult> {
        Ok(InitializeResult::new("Ruskel MCP Server", env!("CARGO_PKG_VERSION"))
            .with_tools(true)
            .with_instructions("Use the 'ruskel' tool to generate Rust API skeletons for crates, modules, or filesystem paths."))
    }

    async fn tools_list(&mut self) -> Result<ListToolsResult> {
        Ok(ListToolsResult::new().with_tool(self.tool_metadata()))
    }

    async fn tools_call(
        &mut self,
        name: String,
        arguments: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        if name == "ruskel" {
            Ok(self.execute_tool(arguments).await)
        } else {
            Err(tenx_mcp::Error::ToolNotFound(name))
        }
    }
}

pub async fn run_mcp_server(
    ruskel: Ruskel,
    addr: Option<String>,
    log_level: Option<String>,
) -> Result<()> {
    // Initialize tracing for TCP mode only
    if addr.is_some() {
        let level = log_level.as_deref().unwrap_or("info");
        let filter = format!("ruskel_mcp={level},tenx_mcp={level}");

        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_writer(std::io::stdout)
            .without_time()
            .init();
    }

    let ruskel_arc = Arc::new(ruskel);
    let server = Server::default()
        .with_connection_factory({
            let ruskel_clone = Arc::clone(&ruskel_arc);
            move || Box::new(RuskelConnection::new(Arc::clone(&ruskel_clone)))
        })
        .with_capabilities(ServerCapabilities::default().with_tools(None));

    match addr {
        Some(addr) => {
            tracing::info!("Starting MCP server on {}", addr);
            server.serve_tcp(addr).await
        }
        None => server.serve_stdio().await,
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RuskelSkeletonTool {
    /// Target to generate - a directory, file path, or a module name
    pub target: String,

    /// Disable default features
    #[serde(default)]
    pub no_default_features: bool,

    /// Enable all features
    #[serde(default)]
    pub all_features: bool,

    /// Specify features to enable
    #[serde(default)]
    pub features: Vec<String>,

    /// Render private items
    #[serde(default)]
    pub private: bool,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::RuskelSkeletonTool;

    #[test]
    fn test_tool_params_deserialization() {
        let params = json!({
            "target": "serde",
            "no_default_features": true,
            "all_features": false
        });

        let result: Result<RuskelSkeletonTool, _> = serde_json::from_value(params);
        assert!(result.is_ok());

        let tool = result.unwrap();
        assert_eq!(tool.target, "serde");
        assert!(tool.no_default_features);
        assert!(!tool.all_features);
    }

    #[test]
    fn test_tool_params_defaults() {
        let params = json!({
            "target": "tokio"
        });

        let result: Result<RuskelSkeletonTool, _> = serde_json::from_value(params);
        assert!(result.is_ok());

        let tool = result.unwrap();
        assert_eq!(tool.target, "tokio");
        assert!(!tool.no_default_features);
        assert!(!tool.all_features);
        assert_eq!(tool.features.len(), 0);
    }

    #[test]
    fn test_tool_params_with_features() {
        let params = json!({
            "target": "tokio",
            "features": ["macros", "rt-multi-thread"],
            "no_default_features": true
        });

        let result: Result<RuskelSkeletonTool, _> = serde_json::from_value(params);
        assert!(result.is_ok());

        let tool = result.unwrap();
        assert_eq!(tool.target, "tokio");
        assert_eq!(tool.features, vec!["macros", "rt-multi-thread"]);
        assert!(tool.no_default_features);
    }

    #[test]
    fn test_tool_params_missing_target() {
        let params = json!({
            "auto_impls": true
        });

        let result: Result<RuskelSkeletonTool, _> = serde_json::from_value(params);
        assert!(result.is_err());
    }
}
