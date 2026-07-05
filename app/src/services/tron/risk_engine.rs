use crate::services::tron::tron_classifier::types::{ClassificationResult, ContractCategory};

pub fn compute_risk_score(
    classification: &ClassificationResult,
    has_swap: bool,
    has_bridge: bool,
    unique_tokens: u16,
    participants: u16,
) -> (u8, String) {
    let mut score = 0u8;

    //
    // category risk
    //
    match classification.category {
        ContractCategory::Mixer => {
            score += 70;
        }

        ContractCategory::Bridge => {
            score += 25;
        }

        ContractCategory::Dex => {
            score += 10;
        }

        ContractCategory::Scam => {
            score += 90;
        }

        _ => {}
    }

    //
    // bridge activity
    //
    if has_bridge {
        score += 25;
    }

    //
    // swaps
    //
    if has_swap {
        score += 10;
    }

    //
    // many tokens
    //
    if unique_tokens >= 3 {
        score += 15;
    }

    //
    // many participants
    //
    if participants >= 10 {
        score += 15;
    }

    //
    // confidence amplification
    //
    let confidence_boost = (classification.confidence * 10.0) as u8;

    score = score.saturating_add(confidence_boost);

    //
    // cap
    //
    if score > 100 {
        score = 100;
    }

    let level = match score {
        0..=24 => "LOW",

        25..=59 => "MEDIUM",

        60..=84 => "HIGH",

        _ => "CRITICAL",
    };

    (score, level.to_string())
}
