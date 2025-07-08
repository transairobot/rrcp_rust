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
pub struct RrcpConfig {
    sc: u32,
    cs: u32,
}
