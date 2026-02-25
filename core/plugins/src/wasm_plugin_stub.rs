#[derive(Clone, Debug, Default)]
pub struct WasmPluginHostStub {
    enabled: bool,
}

impl WasmPluginHostStub {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn load_module(&self, module_name: &str) -> Result<(), String> {
        if !self.enabled {
            return Err(format!(
                "Wasm plugin host disabled; cannot load module '{module_name}'. TODO: integrate runtime sandbox"
            ));
        }
        Err(format!(
            "Wasm plugin host stub enabled but not implemented for module '{module_name}'"
        ))
    }
}
