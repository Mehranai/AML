use std::collections::HashSet;

pub fn compute_exposure_score(
    path_depth: usize,
    touched_sanctioned: bool,
    touched_mixer: bool,
    touched_exchange: bool,
) -> u8 {
    let mut score = 0u8;

    //
    // hop depth
    //
    if path_depth <= 2 {
        score += 40;
    } else if path_depth <= 4 {
        score += 20;
    }

    //
    // sanctioned
    //
    if touched_sanctioned {
        score += 50;
    }

    //
    // mixer
    //
    if touched_mixer {
        score += 40;
    }

    //
    // exchange cashout
    //
    if touched_exchange {
        score += 20;
    }

    score.min(100)
}

pub fn unique_counterparties(addresses: &[String]) -> usize {
    addresses.iter().cloned().collect::<HashSet<_>>().len()
}
