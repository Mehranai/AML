pub fn decay_score(parent_score: f64, hops: u8) -> f64 {
    let hop_penalty = 0.75_f64.powi(hops as i32);

    parent_score * hop_penalty
}
