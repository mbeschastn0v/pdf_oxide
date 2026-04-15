# Benchmark-harness bug-hunt results

Run: `benchmark-harness run --engine pdf-oxide --corpus kreuzberg/pdfs
--ground-truth kreuzberg/gt` (102 stem-matched fixtures, 30 s timeout).

## Cumulative after B1 + B3

| Metric       | v0.3.31 | +B1+B3 |   Δ   |
| ------------ | ------: | -----: | ----: |
| **TF1 mean** |   0.919 | **0.927** | +0.77pp |
| TF1 p50      |   0.965 |  0.965 |     0 |
| **TF1 p10**  |   0.776 | **0.849** | **+7.3pp** |
| SF1 mean     |   0.337 |  0.343 | +0.54pp |
| SF1 p10      |   0.121 |  0.129 | +0.77pp |
| **order mean** |  0.804 | **0.819** | +1.5pp |
| total runtime|   8.3 s |  5.6 s | −33 % |

Zero per-fixture regressions at either fix step.

## Per-fix deltas

### B1 — shared Form XObject with per-page CTM

Symptom: `extract_text(n)` returned page-0 content for every `n` on
PDFs where one Form XObject carries every page's text. Seen on
ExpertPdf output (nougat_005).

| Fixture     | Pre-B1 | Post-B1 |    Δ |
| ----------- | -----: | ------: | ---: |
| nougat_005  |  0.254 |   0.901 | +64.7pp |
| corpus p10  |  0.776 |   0.848 | +7.2pp |

Fix: skip the `xobject_spans_cache` when the current CTM is non-
identity; post-filter extracted spans by page MediaBox.
Branch `fix/b1-linearized-page-resolution`, commit `ab2f49a`.

### B2 — extract_text empty on text-heavy pages

Misdiagnosed. Re-verified post-B1: no fixture has pdf_oxide returning
empty output where pdftotext succeeds. pdfa_010 pages 2/9/11 are
genuinely empty (pdftotext returns empty too). Closed as not-a-bug.

### B3 — first occurrence of running-header dropped

Symptom: when a document's cover-page title repeats on every page as
the running header (common in reports — "Fiscal Year 2010
Appropriations Act", "University of Oklahoma 2009"), the detector
stripped it from every page including page 0.

Fix: track first-seen page per signature; keep the first, mark only
subsequent appearances as Pagination artifacts.
Branch `fix/b3-running-artifact-overreach`, commit `706d954`.

| Metric     | Pre-B3 | Post-B3 |    Δ |
| ---------- | -----: | ------: | ---: |
| TF1 mean   |  0.925 |   0.927 | +0.16pp |
| SF1 mean   |  0.339 |   0.343 | +0.33pp |
| order mean |  0.808 |   0.819 | +1.04pp |

### B4 — reading-order degradation on multi-column / dashboard pages

Deferred — architectural change. `extract_text` currently uses
`row_aware_span_cmp` (Y-band descending, X ascending) which breaks on
multi-column text. XY-cut exists in `src/pipeline/reading_order/xycut.rs`
but isn't the default for `extract_text`.

Worst offenders post-B1+B3 (order_score):

| Fixture     | order | TF1  |
| ----------- | ----: | ---: |
| nougat_026  | 0.39  | 0.81 |
| pdfa_001    | 0.44  | 0.81 |
| pdfa_027    | 0.45  | 0.93 |

Wiring XY-cut as the default reading order is the right long-term
fix; scope too big for this session without full corpus validation.
Filed for follow-up.

## Remaining gap vs pdftotext

|              | pdf_oxide (post) | pdftotext |   Δ  |
| ------------ | ---------------: | --------: | ---: |
| TF1 mean     |            0.927 |     0.946 | -1.9 |
| TF1 p10      |            0.849 |     0.881 | -3.2 |
| order mean   |            0.819 |     0.863 | -4.4 |

All three gaps narrowed from the baseline. The remaining TF1 gap is
mostly B4-territory (reading-order scrambling content on complex
layouts) plus font-parsing edge cases that surface as warnings on a
handful of fixtures (`cmap format 0` unsupported).

## Validation workflow (proved end-to-end)

1. Run the harness → compute TF1/SF1 against ground truth.
2. Diff aggregates vs `pdftotext` (and over time, docling / pdfium).
3. Drill into worst fixtures to find real bugs.
4. Fix + add TDD regression test in `tests/`.
5. Rerun harness; `benchmark-harness diff` asserts no regression.
6. Commit with before/after numbers.

Every step went through real code on this corpus — nougat_005 went
from 0.254 → 0.901 TF1 because the harness surfaced a bug nobody had
caught in byte-diff or unit-test territory.

## Reproduce

```bash
make benchmark-fetch

# baseline
git checkout v0.3.31
cargo build --release -p benchmark-harness
make benchmark-run OUTPUT=v0.3.31.json

# with fixes
git checkout fix/b3-running-artifact-overreach
cargo build --release -p benchmark-harness
make benchmark-run OUTPUT=head.json

make benchmark-compare BASE=v0.3.31.json HEAD=head.json
```
