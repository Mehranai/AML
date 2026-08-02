#[derive(Debug, Clone)]
pub struct ExchangeAttribution {
    pub exchange_name: String,
    pub role: String,
    pub confidence: f32,
    pub detection_source: String,
    pub cluster_id: Option<String>,
}
