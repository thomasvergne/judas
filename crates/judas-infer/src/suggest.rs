/// Levenshtein distance calculation for typo suggestion
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut distances: Vec<usize> = (0..=b_chars.len()).collect();

    for (i, &ca) in a_chars.iter().enumerate() {
        let mut new_distances = vec![i + 1; b_chars.len() + 1];
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            new_distances[j + 1] = (distances[j + 1] + 1)
                .min(new_distances[j] + 1)
                .min(distances[j] + cost);
        }
        distances = new_distances;
    }
    distances[b_chars.len()]
}

/// Find the closest candidate string to `target` within an
/// acceptable edit distance.
pub fn find_closest_candidate<'a>(
    target: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let mut best: Option<(&'a str, usize)> = None;
    for cand in candidates {
        let dist = levenshtein_distance(target, cand);
        let max_allowed = (target.len() / 2).clamp(1, 3);
        if dist <= max_allowed {
            if let Some((_, best_dist)) = best {
                if dist < best_dist {
                    best = Some((cand, dist));
                }
            } else {
                best = Some((cand, dist));
            }
        }
    }
    best.map(|(cand, _)| cand.to_string())
}
