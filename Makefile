.PHONY: all check test lint pipeline validate bench snapshots update-snapshots clean mcp-dev viewer-dev mcp-build viewer-build metrics validate-strict

all: check test pipeline validate

check:
	cargo check --all-targets

test:
	cargo test --all
	cargo test --test pipeline_integration
	cargo test --test snapshot_tests

lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --all -- --check

pipeline:
	cargo run --bin tilegraph -- generate-synth
	cargo run --bin tilegraph -- build-tiles
	cargo run --bin tilegraph -- build-graph

validate:
	cargo run --bin tilegraph -- validate

bench:
	cargo run --bin tilegraph -- benchmark

## Run full pipeline with Prometheus metrics collection; output written to output/reports/metrics.txt
metrics:
	cargo run --features tilegraph-metrics --bin tilegraph -- generate-synth
	cargo run --features tilegraph-metrics --bin tilegraph -- build-tiles
	cargo run --features tilegraph-metrics --bin tilegraph -- build-graph
	cargo run --features tilegraph-metrics --bin tilegraph -- validate
	@echo "Metrics summary:"
	@cat output/reports/metrics.txt 2>/dev/null || echo "(no metrics file found)"

## Run validate with spec-compliance --strict checks
validate-strict:
	cargo run --bin tilegraph -- validate --strict

snapshots:
	cargo test --test snapshot_tests

update-snapshots:
	bash scripts/update_snapshots.sh

clean:
	cargo clean
	rm -rf output/synth/ output/tiles/ output/graph/ output/reports/
	rm -f output/.build_manifest.json

mcp-dev:
	cd apps/tilegraphmcp && npm run dev

viewer-dev:
	cd apps/tilegraphviewer && npm run dev

mcp-build:
	cd apps/tilegraphmcp && npm run build

viewer-build:
	cd apps/tilegraphviewer && npm run build
