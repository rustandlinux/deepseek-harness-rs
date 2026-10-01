use crate::tool::Tool;
use std::collections::HashMap;
use std::sync::Arc;

/// A thread-safe container registering and dispatching available tools.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tool into the registry.
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
    }

    /// Register a shared reference to a tool.
    pub fn register_arc(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Retrieve a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Return an ordered list of registered tool names.
    pub fn names(&self) -> Vec<String> {
        let mut list: Vec<_> = self.tools.keys().cloned().collect();
        list.sort();
        list
    }

    /// Export all registered tools as OpenAI-compatible schema JSON definitions.
    pub fn export_openai_tools(&self) -> Vec<serde_json::Value> {
        let mut schemas = Vec::new();
        for name in self.names() {
            if let Some(tool) = self.tools.get(&name) {
                schemas.push(tool.schema().to_openai_value());
            }
        }
        schemas
    }
}

