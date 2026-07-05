pub fn boost_confidence(
    base: f32,
    protocol_match: bool,
    method_match: bool,
    flow_match: bool,
) -> f32 {
    let mut confidence = base;

    if protocol_match {
        confidence += 0.30;
    }

    if method_match {
        confidence += 0.20;
    }

    if flow_match {
        confidence += 0.10;
    }

    confidence.min(1.0)
}
