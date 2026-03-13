# Repository Guidelines

## Agent Workflow
All developers and coding agents working in this repository must read and follow this file before making changes.

- Keep changes scoped and single-purpose.
- Prefer editing tracked source files only; do not commit generated outputs.
- Before committing code changes, run the relevant validation commands for the area you touched.
- Use `--output <dir>` for `lumberjacks` runs so repo-local experiment artifacts are not left behind.
- Leave local-only machine or tool configuration out of commits unless the change is intentionally repo-wide.

## Project Structure & Module Organization
The root `Cargo.toml` defines a three-crate Rust workspace with historical directory names:

- `npc-engine-core/`: core planner crate published in-workspace as `bioma-npc-core`
- `npc-engine-utils/`: helper/executor crate published in-workspace as `bioma-npc-utils`
- `scenario-lumberjacks/`: demo/research scenario package `lumberjacks`

`npc-engine-core/` contains the MCTS engine in `src/`, tests in `tests/`, benches in `benches/`, and examples in `examples/`. `npc-engine-utils/` provides executors and helpers. `scenario-lumberjacks/` contains the lumberjacks scenario, with code in `src/`, binaries in `src/bin/`, and data in `assets/`, `maps/`, `configs/`, and `experiments/`.

## Build, Test, and Development Commands
- `cargo check --all-targets`: workspace compile check; matches the expected baseline validation.
- `cargo test --all-targets`: runs libraries, tests, examples, and benches.
- `cargo fmt --all`: formats the workspace. CI enforces `cargo fmt --all -- --check`.
- `cargo clippy -- -D warnings`: lint gate used for merge-ready changes.
- `cargo run -p bioma-npc-core --example capture`: ANSI text trace of planning/execution; also writes Graphviz `.dot` files to `/tmp/capture_graphs/`.
- `cargo run -p bioma-npc-core --example learn`: non-interactive 600-epoch run that prints one score per epoch.
- `printf '0 0\nq\n' | cargo run -p bioma-npc-core --features graphviz --example tic-tac-toe`: quick startup check for the interactive example.
- `TERM=xterm cargo run -p bioma-npc-core --example ecosystem`: ecosystem example; `TERM=xterm` avoids `clearscreen` panics in minimal terminals.
- `cargo run -p lumberjacks --bin lumberjacks -- --batch -s turns=10 -s mcts.visits=100 --output /tmp/lumber-smoke scenario-lumberjacks/experiments/base.json`: practical headless smoke test for the lumberjacks scenario.

On Linux, `lumberjacks` needs `libudev-dev` and `libasound2-dev`. Its first build is much slower than the core examples because it compiles the Bevy renderer/audio stack even in headless mode.

## Coding Style & Naming Conventions
`npc-engine-core` and `npc-engine-utils` target Rust 2021, while `scenario-lumberjacks` targets Rust 2024. Follow `rustfmt` defaults with 4-space indentation. Use `UpperCamelCase` for types and traits, `snake_case` for modules, files, functions, and tests, and `SCREAMING_SNAKE_CASE` for constants.

Keep planner abstractions in `npc-engine-core`, shared helpers in `npc-engine-utils`, and scenario logic in `scenario-lumberjacks`. When referring to crates in code, use the current crate names `bioma_npc_core` and `bioma_npc_utils`.

## Testing Guidelines
Add regression coverage in the crate you change; planner behavior usually belongs in `npc-engine-core/tests/*_tests.rs`. Seed MCTS configs for deterministic assertions.

For stochastic examples such as `learn`, check trends or bounds rather than exact output. A healthy run should trend upward over time rather than match exact epoch values.

## Runtime Findings
`capture` is useful for inspecting task scheduling and invalidation, but it is not a quick finite smoke test.

`lumberjacks` writes artifacts by default, including:

- `scenario-lumberjacks/experiments/info.json`
- run folders such as `scenario-lumberjacks/experiments/0/run.json`
- serialized maps when analytics are enabled

Use `--output <dir>` or clean generated files before committing. Also note that `scenario-lumberjacks/experiments/base.json` does not define the top-level `turns` field batch mode expects, so pass `-s turns=<n>` for headless runs.

## Commit & Pull Request Guidelines
Use short, imperative commit subjects. Keep commits single-purpose.

For PRs or merge-ready branches:

- list the commands you ran
- note generated artifacts or cleanup
- link relevant issue, design, or provenance context
- include screenshots only when `scenario-lumberjacks` visuals change
