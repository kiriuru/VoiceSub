//! SST `local_asr_constants` parity — shared local ASR tuning constants.

/// Typical Parakeet silence hallucinations (SST `SHORT_HALLUCINATION_TOKENS`).
pub const SHORT_HALLUCINATION_TOKENS: &[&str] = &[
    "yeah",
    "yeah.",
    "mm-hmm",
    "mm-hmm.",
    "mhm",
    "mhm.",
    "uh-huh",
    "uh-huh.",
    "okay",
    "okay.",
    "ok",
    "ok.",
    "hmm",
    "hmm.",
    "uh",
    "uh.",
    "ah",
    "ah.",
    "yep",
    "yep.",
    "nope",
    "nope.",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_count_matches_legacy_list() {
        assert_eq!(SHORT_HALLUCINATION_TOKENS.len(), 22);
    }
}
