//! Engine adapters.
//!
//! Each engine extracts a PDF to markdown. The trait intentionally
//! carries a `name()` and a single `extract` method so we can add
//! subprocess-based adapters (pdftotext, pdfium, docling) without
//! touching the runner.

use anyhow::{Context, Result};
use clap::ValueEnum;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum EngineKind {
    PdfOxide,
    // Populated in later phases:
    // Pdftotext,
    // Pdfium,
    // Docling,
}

pub struct Extraction {
    pub markdown: String,
    pub duration: Duration,
}

pub trait Engine {
    fn name(&self) -> &'static str;
    fn extract(&self, pdf: &Path) -> Result<Extraction>;
}

pub fn build(kind: EngineKind) -> Box<dyn Engine> {
    match kind {
        EngineKind::PdfOxide => Box::new(PdfOxideEngine),
    }
}

pub struct PdfOxideEngine;

impl Engine for PdfOxideEngine {
    fn name(&self) -> &'static str {
        "pdf_oxide"
    }

    fn extract(&self, pdf: &Path) -> Result<Extraction> {
        use pdf_oxide::PdfDocument;
        let start = Instant::now();
        let mut doc = PdfDocument::open(pdf).with_context(|| format!("open {}", pdf.display()))?;
        let page_count = doc.page_count().unwrap_or(0);
        let mut md = String::new();
        for page in 0..page_count {
            // Text-only for now. Phase 3 swaps to the markdown converter
            // so SF1 can score block structure.
            let Ok(text) = doc.extract_text(page) else {
                continue;
            };
            md.push_str(&text);
            md.push('\n');
        }
        Ok(Extraction {
            markdown: md,
            duration: start.elapsed(),
        })
    }
}
