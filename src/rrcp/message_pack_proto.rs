use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct SensorData {
    pub servos: Vec<ServoStatus>,
    pub images: Vec<Image>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct ServoStatus {
    pub angle: f64,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Action {
    pub ts: u64,
    pub actions: Vec<f64>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct WasmModule {
    pub name: String,

    #[serde(with = "serde_bytes")]
    pub wasm: Vec<u8>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct RrcpConfig {
    pub modules: Vec<WasmModule>,
}

impl RrcpConfig {
    pub fn get_main_wasm(&self) -> Option<&[u8]> {
        for module in &self.modules {
            if module.name == "main" {
                return Some(&module.wasm);
            }
        }
        None
    }
}
