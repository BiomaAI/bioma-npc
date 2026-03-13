# bioma-npc

`bioma-npc` is a fork of [`npc-engine`](https://github.com/ethz-gtc/npc-engine) designed for agentic MMOs and large-scale multi-agent simulation systems.

BiomaAI is using this codebase as the foundation for commercial agent systems, while retaining clear provenance for inherited upstream work and research artifacts.

## What It Includes

* `bioma-npc-core`: the MCTS planner and core abstractions.
* `bioma-npc-utils`: executors, helper types, graph tooling, and lightweight learning utilities.
* `lumberjacks`: the upstream research scenario/demo packaged in `scenario-lumberjacks/`.

The current directory layout still uses the historical workspace folders `npc-engine-core/`, `npc-engine-utils/`, and `scenario-lumberjacks/`.

## Getting It

Clone the BiomaAI repository:

```bash
git clone git@github.com:BiomaAI/bioma-npc.git
```

If you plan to use the lumberjacks assets, install Git LFS first. Without it, PNG assets may not be fetched correctly.

## Workspace Commands

```bash
cargo check --all-targets
cargo test --all-targets
cargo clippy -- -D warnings
cargo fmt --all
```

## Examples

### Tic-tac-toe

```bash
cargo run --release -p bioma-npc-core --features graphviz --example tic-tac-toe
```

Interactive board-game example for validating the planner loop.

Source directory: [`npc-engine-core/examples/tic-tac-toe`](npc-engine-core/examples/tic-tac-toe/)

### Capture

```bash
cargo run --release -p bioma-npc-core --example capture
```

Competitive multi-agent capture simulation with variable-duration tasks and world-agent bookkeeping.

Source directory: [`npc-engine-core/examples/capture`](npc-engine-core/examples/capture/)

### Learn

```bash
cargo run --release -p bioma-npc-core --example learn
```

One-dimensional woodcutting example that trains a lightweight value estimator over repeated runs.

Plotting helper:

```bash
npc-engine-core/examples/learn/plot.py
```

Source directory: [`npc-engine-core/examples/learn`](npc-engine-core/examples/learn/)

### Ecosystem

```bash
cargo run --release -p bioma-npc-core --example ecosystem
```

Open-ended predator/prey simulation with threaded planning, partial observability, and population dynamics.

Statistics plotting helper:

```bash
npc-engine-core/examples/ecosystem/plot_ecosystem_stats.py
```

Source directory: [`npc-engine-core/examples/ecosystem`](npc-engine-core/examples/ecosystem/)

### Lumberjacks

```bash
cargo run --release -p lumberjacks --bin lumberjacks -- scenario-lumberjacks/experiments/base.json
```

The lumberjacks scenario remains bundled as the upstream research demo. In interactive mode, press `Enter` to advance turns.

Useful headless smoke test:

```bash
cargo run --release -p lumberjacks --bin lumberjacks -- --batch -s turns=10 scenario-lumberjacks/experiments/base.json
```

Source directory: [`scenario-lumberjacks`](scenario-lumberjacks/)

## Documentation

Generate local API docs for the reusable crates with:

```bash
cargo doc --open -p bioma-npc-core -p bioma-npc-utils
```

For good runtime performance, prefer `--release` builds when running simulations.

## Search Tree Graphs

Some examples emit Graphviz `.dot` files to a temporary directory. Convert them to PDFs with:

```bash
for file in *.dot; do dot -Tpdf "$file" -o "${file%.dot}.pdf"; done
```

## Commercial Use & License

This repository is distributed by BiomaAI under the proprietary terms described in [LICENSE](LICENSE).

It also contains code and materials derived from upstream `npc-engine`. Preserved upstream MIT licensing information remains available in [LICENSE-MIT](LICENSE-MIT), and fork provenance is documented in [PROVENANCE.md](PROVENANCE.md).

## Fork Origin

`bioma-npc` began from the `npc-engine` project developed around the ETH Game Technology Center research effort:

* Upstream repository: <https://github.com/ethz-gtc/npc-engine>
* Upstream research scenario in this repo: [`scenario-lumberjacks`](scenario-lumberjacks/)
* Contributor credits retained in [AUTHORS.txt](AUTHORS.txt)

BiomaAI is adapting this foundation for agentic MMO use cases and future proprietary systems while preserving attribution for inherited upstream work.

## Citation

If you use the inherited research work in an academic context, cite the original paper:

```text
@inproceedings{raymond2020leveraging,
  title={Leveraging efficient planning and lightweight agent definition: a novel path towards emergent narrative},
  author={Raymond, Henry and Knobloch, Sven and Z{\"u}nd, Fabio and Sumner, Robert W and Magnenat, St{\'e}phane},
  booktitle={12th Intelligent Narrative Technolgies Workshop, held with the AIIDE Conference (INT10 2020)},
  doi={10.3929/ethz-b-000439084},
  year={2020},
}
```

## Acknowledgments

Thanks to the original `npc-engine` contributors and research collaborators, including Patrick Eppensteiner, Nora Tommila, and Heinrich Grattenthaler.
