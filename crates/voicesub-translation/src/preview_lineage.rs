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
