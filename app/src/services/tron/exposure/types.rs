#[derive(Debug, Clone)]
pub struct ExposureNode {
    pub address: String,
    pub score: f64,
    pub hops: u8,
}
