//! Run-and-diff: drive engines across a corpus, emit a JSON report,
//! compare two reports and gate on regression.

use crate::engine::{self, Engine};
use crate::score;
use crate::{DiffArgs, RunArgs};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct FixtureResult {
    pub name: String,
    pub tf1: Option<f64>,
    pub duration_ms: Option<u128>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Aggregate {
    pub count: usize,
    pub ok: usize,
    pub tf1_mean: f64,
    pub tf1_p50: f64,
    pub tf1_p90: f64,
    pub duration_ms_total: u128,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Report {
    pub engine: String,
    pub corpus: PathBuf,
    pub ground_truth: PathBuf,
    pub fixtures: Vec<FixtureResult>,
    pub aggregate: Aggregate,
}

pub fn run(args: RunArgs) -> Result<()> {
    let engine = engine::build(args.engine);
    log::info!("engine = {}", engine.name());

    let pairs = collect_pairs(&args.corpus, &args.ground_truth)?;
    if pairs.is_empty() {
        return Err(anyhow!(
            "no PDF/markdown pairs found — expected matching *.pdf under {} \
             and *.md under {}",
            args.corpus.display(),
            args.ground_truth.display()
        ));
    }
    log::info!("found {} fixture pairs", pairs.len());

    let mut fixtures = Vec::with_capacity(pairs.len());
    for (i, (pdf, gt_path)) in pairs.iter().enumerate() {
        log::info!("[{}/{}] {}", i + 1, pairs.len(), pdf.display());
        fixtures.push(score_one(&*engine, pdf, gt_path));
    }

    let aggregate = aggregate(&fixtures);
    let report = Report {
        engine: engine.name().to_string(),
        corpus: args.corpus,
        ground_truth: args.ground_truth,
        fixtures,
        aggregate,
    };
    fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    log::info!(
        "wrote {} — mean TF1 {:.3} across {} fixtures ({} ok)",
        args.output.display(),
        report.aggregate.tf1_mean,
        report.aggregate.count,
        report.aggregate.ok
    );
    Ok(())
}

fn score_one(engine: &dyn Engine, pdf: &Path, gt_path: &Path) -> FixtureResult {
    let name = pdf
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match engine.extract(pdf) {
        Ok(ext) => {
            let gt = match fs::read_to_string(gt_path) {
                Ok(s) => s,
                Err(e) => {
                    return FixtureResult {
                        name,
                        tf1: None,
                        duration_ms: Some(ext.duration.as_millis()),
                        error: Some(format!("ground-truth read: {e}")),
                    };
                },
            };
            FixtureResult {
                name,
                tf1: Some(score::tf1(&ext.markdown, &gt)),
                duration_ms: Some(ext.duration.as_millis()),
                error: None,
            }
        },
        Err(e) => FixtureResult {
            name,
            tf1: None,
            duration_ms: None,
            error: Some(e.to_string()),
        },
    }
}

fn aggregate(rs: &[FixtureResult]) -> Aggregate {
    let mut tf1s: Vec<f64> = rs.iter().filter_map(|r| r.tf1).collect();
    tf1s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = if tf1s.is_empty() {
        0.0
    } else {
        tf1s.iter().sum::<f64>() / tf1s.len() as f64
    };
    let p = |q: f64| -> f64 {
        if tf1s.is_empty() {
            0.0
        } else {
            let idx = ((tf1s.len() as f64 - 1.0) * q).round() as usize;
            tf1s[idx.min(tf1s.len() - 1)]
        }
    };
    Aggregate {
        count: rs.len(),
        ok: tf1s.len(),
        tf1_mean: mean,
        tf1_p50: p(0.50),
        tf1_p90: p(0.10), // lower-tail quality percentile
        duration_ms_total: rs.iter().filter_map(|r| r.duration_ms).sum(),
    }
}

/// Match by file stem: `foo.pdf` ↔ `foo.md`.
fn collect_pairs(corpus: &Path, gt: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut gt_map: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in walkdir::WalkDir::new(gt) {
        let entry = entry.with_context(|| format!("walk {}", gt.display()))?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "md") {
            let stem = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            gt_map.insert(stem, entry.path().to_path_buf());
        }
    }
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(corpus) {
        let entry = entry.with_context(|| format!("walk {}", corpus.display()))?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "pdf") {
            let stem = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if let Some(gt_path) = gt_map.get(&stem) {
                out.push((entry.path().to_path_buf(), gt_path.clone()));
            }
        }
    }
    Ok(out)
}

pub fn diff(args: DiffArgs) -> Result<()> {
    let base: Report = serde_json::from_slice(&fs::read(&args.base)?)?;
    let head: Report = serde_json::from_slice(&fs::read(&args.head)?)?;

    println!("engine={} corpus={}", base.engine, base.corpus.display());
    println!(
        "mean TF1  base={:.3}  head={:.3}  Δ={:+.3}pp",
        base.aggregate.tf1_mean,
        head.aggregate.tf1_mean,
        (head.aggregate.tf1_mean - base.aggregate.tf1_mean) * 100.0,
    );

    let base_map: BTreeMap<&str, &FixtureResult> =
        base.fixtures.iter().map(|f| (f.name.as_str(), f)).collect();
    let mut worst: Vec<(&str, f64, f64, f64)> = Vec::new();
    for h in &head.fixtures {
        let Some(b) = base_map.get(h.name.as_str()) else {
            continue;
        };
        let (Some(bt), Some(ht)) = (b.tf1, h.tf1) else {
            continue;
        };
        let delta_pp = (ht - bt) * 100.0;
        if delta_pp < 0.0 {
            worst.push((h.name.as_str(), bt, ht, delta_pp));
        }
    }
    worst.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
    let show = worst.iter().take(10);
    println!("worst fixture regressions:");
    for (n, bt, ht, d) in show {
        println!("  {:<40} {:.3} → {:.3}  ({:+.2}pp)", n, bt, ht, d);
    }

    let mean_drop_pp = (base.aggregate.tf1_mean - head.aggregate.tf1_mean) * 100.0;
    let worst_drop_pp = worst.first().map(|w| -w.3).unwrap_or(0.0);
    if mean_drop_pp > args.mean_tf1_drop_pp {
        return Err(anyhow!(
            "mean TF1 dropped {mean_drop_pp:.2}pp (gate: {:.2}pp)",
            args.mean_tf1_drop_pp
        ));
    }
    if worst_drop_pp > args.per_fixture_tf1_drop_pp {
        return Err(anyhow!(
            "worst fixture dropped {worst_drop_pp:.2}pp (gate: {:.2}pp)",
            args.per_fixture_tf1_drop_pp
        ));
    }
    println!("no regression above thresholds.");
    Ok(())
}
