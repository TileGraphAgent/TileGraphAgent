.PHONY: all check test lint pipeline validate bench snapshots update-snapshots clean mcp-dev viewer-dev mcp-build viewer-build

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

snapshots:
	cargo test --test snapshot_tests

update-snapshots:
	bash scripts/update_snapshots.sh

clean:
	cargo clean
	rm -rf output/synth/ output/tiles/ output/graph/ output/reports/
	rm -f output/.build_manifest.json

mcp-dev:
	cd apps/tilegraph-mcp-server && npm run dev

viewer-dev:
	cd apps/tilegraph-viewer && npm run dev

mcp-build:
	cd apps/tilegraph-mcp-server && npm run build

viewer-build:
	cd apps/tilegraph-viewer && npm run build
