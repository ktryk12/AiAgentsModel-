use std::{fs::File, io::{BufRead, BufReader}};
use serde::Deserialize;

#[derive(Deserialize)]
struct Example {
    prompt: String,
    completion: String,
}

use crate::types_training::QualityReport;

#[derive(Clone, Debug)]
pub struct ValidationStats {
    pub examples: u64,
    pub dataset_hash: [u8; 32],
    pub quality: QualityReport,
}

pub fn validate_jsonl_and_hash(path: &std::path::Path) -> Result<ValidationStats, Vec<String>> {
    let f = File::open(path).map_err(|e| vec![format!("IO: {e}")])?;
    let reader = BufReader::new(f);

    let mut errors: Vec<String> = vec![];
    let mut hasher = blake3::Hasher::new();

    let mut count: u64 = 0;
    let mut prompt_sum: u64 = 0;
    let mut completion_sum: u64 = 0;
    let mut too_short: u64 = 0;

    // simple duplicate detection
    let mut seen = std::collections::HashSet::<[u8; 32]>::new();
    let mut dupes: u64 = 0;

    for (i, line) in reader.lines().enumerate() {
        let line_no = i + 1;
        let line = match line {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("Line {line_no}: IO read error: {e}"));
                continue;
            }
        };

        // Hash exact content + newline => stable hash across reads
        hasher.update(line.as_bytes());
        hasher.update(b"\n");

        if line.trim().is_empty() {
            errors.push(format!("Line {line_no}: empty line"));
            continue;
        }

        let ex: Example = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("Line {line_no}: invalid JSON: {e}"));
                continue;
            }
        };

        let p = ex.prompt.trim();
        let c = ex.completion.trim();

        if p.is_empty() {
            errors.push(format!("Line {line_no}: empty prompt"));
            continue;
        }
        if c.is_empty() {
            errors.push(format!("Line {line_no}: empty completion"));
            continue;
        }

        // minimal “not garbage” heuristics
        if p.len() < 10 || c.len() < 5 {
            too_short += 1;
        }

        // duplicate fingerprint
        let fp = blake3::hash(format!("{p}\n{c}").as_bytes()).into();
        if !seen.insert(fp) {
            dupes += 1;
        }

        count += 1;
        prompt_sum += p.len() as u64;
        completion_sum += c.len() as u64;
    }

    if !errors.is_empty() {
        // hard fail: invalid format (Week 1 rule)
        return Err(errors);
    }
    if count == 0 {
        return Err(vec!["No valid examples found".to_string()]);
    }

    let dataset_hash: [u8; 32] = hasher.finalize().into();

    let avg_prompt = (prompt_sum / count) as u32;
    let avg_completion = (completion_sum / count) as u32;
    let duplicate_rate = (dupes as f32) / (count as f32);

    // quality scoring (simple, explainable)
    let mut score: i32 = 100;
    let mut warnings: Vec<String> = vec![];
    let hard_errors: Vec<String> = vec![];

    if count < 200 {
        score -= 25;
        warnings.push(format!("Low example count ({count}). Recommended: 200+"));
    }
    if duplicate_rate > 0.15 {
        score -= 25;
        warnings.push(format!("High duplicate_rate ({duplicate_rate:.2}). Consider deduping"));
    }
    if too_short > (count / 5) {
        score -= 20;
        warnings.push(format!("Many very short examples ({too_short}). Risk of low-quality LoRA"));
    }
    if avg_completion < 20 {
        score -= 10;
        warnings.push(format!("Avg completion length is low ({avg_completion}). Model may learn short replies"));
    }

    let score = score.clamp(0, 100) as u8;

    Ok(ValidationStats {
        examples: count,
        dataset_hash,
        quality: QualityReport {
            score,
            warnings,
            hard_errors,
            duplicate_rate,
            avg_prompt_len: avg_prompt,
            avg_completion_len: avg_completion,
            too_short_count: too_short,
        }
    })
}
