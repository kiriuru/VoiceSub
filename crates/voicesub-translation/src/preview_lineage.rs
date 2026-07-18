use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct TranslationPreviewLineage {
    generations: HashMap<String, u64>,
}

impl TranslationPreviewLineage {
    pub fn lineage_key(segment_id: Option<&str>, revision: Option<u64>) -> Option<String> {
        let segment = segment_id?.trim();
        let revision = revision?;
        if segment.is_empty() {
            return None;
        }
        Some(format!("{segment}:{revision}"))
    }

    pub fn supersede(&mut self, key: Option<&str>) -> u64 {
        let Some(key) = key.filter(|k| !k.is_empty()) else {
            return 0;
        };
        let entry = self.generations.entry(key.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }

    pub fn generation(&self, key: &str) -> u64 {
        self.generations.get(key).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supersede_increments_generation_for_same_key() {
        let mut lineage = TranslationPreviewLineage::default();
        assert_eq!(lineage.supersede(Some("seg:1")), 1);
        assert_eq!(lineage.supersede(Some("seg:1")), 2);
        assert_eq!(lineage.generation("seg:1"), 2);
    }

    #[test]
    fn generation_counters_are_never_reset() {
        let mut lineage = TranslationPreviewLineage::default();
        for index in 0..300 {
            let key = format!("seg:{index}");
            assert_eq!(lineage.supersede(Some(&key)), 1);
        }
        // Re-superseding an early key must continue from 1 → 2, not restart at 1.
        assert_eq!(lineage.supersede(Some("seg:0")), 2);
        assert_eq!(lineage.generation("seg:0"), 2);
    }
}
