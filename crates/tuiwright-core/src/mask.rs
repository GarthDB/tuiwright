//! Regex masking for volatile cell content before baseline diffs.

use regex::Regex;

use crate::snapshot::SnapshotGrid;

/// Compiled ignore patterns from config.
#[derive(Debug, Clone, Default)]
pub struct DiffMasks {
    patterns: std::sync::Arc<Vec<Regex>>,
}

impl DiffMasks {
    /// Compile patterns from config strings. Invalid patterns are skipped with a stderr note.
    pub fn compile(raw: &[String]) -> Self {
        let mut patterns = Vec::new();
        for pat in raw {
            match Regex::new(pat) {
                Ok(re) => patterns.push(re),
                Err(err) => eprintln!("tuiwright: ignoring invalid diff.ignore_patterns {pat:?}: {err}"),
            }
        }
        Self {
            patterns: std::sync::Arc::new(patterns),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn mask_symbol(&self, symbol: &str) -> String {
        if self.patterns.is_empty() {
            return symbol.to_string();
        }
        let mut out = symbol.to_string();
        for re in self.patterns.iter() {
            out = re.replace_all(&out, "*").into_owned();
        }
        out
    }

    pub fn apply_to_grid(&self, grid: &SnapshotGrid) -> SnapshotGrid {
        if self.patterns.is_empty() {
            return grid.clone();
        }
        let mut masked = grid.clone();
        for cell in &mut masked.cells {
            cell.symbol = self.mask_symbol(&cell.symbol);
        }
        masked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_harness_session_id() {
        let masks = DiffMasks::compile(&[r"hs-\S+".to_string()]);
        assert_eq!(masks.mask_symbol("hs-123-0-999"), "*");
        assert_eq!(masks.mask_symbol("idle hs-probe ok"), "idle * ok");
    }
}
