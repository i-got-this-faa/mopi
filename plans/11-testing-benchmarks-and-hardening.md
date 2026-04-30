# Testing, Benchmarks, And Hardening

Overall Status: NOT STARTED  
Current Owner: UNASSIGNED  
Blocked By: Core implementation across workstreams  
Last Updated: 2026-04-27

## Objective

Create the test matrix, performance harnesses, adversarial fixtures, and safety validation needed to prove the engine is correct, fast, and resilient under realistic and hostile local-file conditions.

## Scope

- unit tests
- integration tests
- fixture corpus
- malformed document tests
- symlink and policy tests
- benchmarks
- fuzzing and soak testing

## Fixture Corpus Requirements

- [ ] plain text files
- [ ] config files across supported formats
- [ ] code files
- [ ] realistic `docx` files
- [ ] realistic `odt` files
- [ ] realistic `pdf` files
- [ ] malformed `docx`, `odt`, and `pdf` samples
- [ ] symlink loops and duplicate aliases
- [ ] hidden files and hidden directories
- [ ] mixed whitelist and blacklist cases

## Test Categories

- [ ] config parsing and validation tests
- [ ] crawler traversal and identity tests
- [ ] extractor format tests
- [ ] extractor malformed-input tests
- [ ] storage migration and recovery tests
- [ ] lexical search correctness tests
- [ ] semantic retrieval regression tests
- [ ] hybrid ranking golden tests
- [ ] CLI integration tests
- [ ] daemon lifecycle tests
- [ ] GUI smoke or integration tests where practical

## Benchmark Categories

- [ ] crawl throughput
- [ ] extraction throughput by format
- [ ] lexical query latency
- [ ] query embedding latency
- [ ] hybrid query latency
- [ ] indexing throughput for changed-only refresh
- [ ] daemon warm-start and cold-start costs
- [ ] memory footprint with model loaded

## Hardening Checklist

- [ ] Test hidden-file defaults thoroughly.
- [ ] Test whitelist and blacklist boundaries thoroughly.
- [ ] Test symlink loops and duplicate alias handling.
- [ ] Test oversized files and configured extraction caps.
- [ ] Test malformed archives and PDFs.
- [ ] Test interrupted indexing and recovery.
- [ ] Test daemon behavior with missing model artifacts.
- [ ] Fuzz parser entrypoints or input normalization paths where practical.

## Acceptance Criteria

- [ ] The project has enough tests to refactor confidently across all critical subsystems.
- [ ] Benchmarks exist for every latency-sensitive stage.
- [ ] Security and resilience regressions are covered by automated tests.
- [ ] Release gating can be based on evidence rather than manual confidence.

## Verification

- [ ] Run the full automated test suite.
- [ ] Run the benchmark suite and record baseline numbers.
- [ ] Run hardening scenarios and confirm no crashes or corrupt state.

## Notes And Risks

- Performance claims without repeatable benchmarks do not count.
- Hardening is not a late polish phase. It must validate assumptions made in crawl, extract, and storage design.
