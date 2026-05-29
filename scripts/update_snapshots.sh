#!/usr/bin/env bash
set -e

echo "Rebuilding pipeline..."
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles

echo "Updating snapshots..."
python3 - <<'EOF'
import json, os

objs = json.load(open("output/synth/objects.json"))
open("tests/snapshots/objects_count.txt", "w").write(str(len(objs)))

idx = json.load(open("output/tiles/index/spatial_index.json"))
open("tests/snapshots/spatial_index_count.txt", "w").write(str(idx["record_count"]))

def count_tiles(tile):
    return 1 + sum(count_tiles(c) for c in tile.get("children", []))
ts = json.load(open("output/tiles/tileset.json"))
open("tests/snapshots/tileset_tile_count.txt", "w").write(str(count_tiles(ts["root"])))

ft = json.load(open("output/tiles/metadata/tile_feature_map.json"))
open("tests/snapshots/feature_table_count.txt", "w").write(str(len(ft["mappings"])))

props = json.load(open("output/tiles/metadata/object_properties.json"))
pump = next((p for p in props if p.get("tag") == "P-10101"), None)
if pump:
    open("tests/snapshots/p10101_tag.txt", "w").write(pump.get("tag", ""))

print("Snapshots updated:")
for f in sorted(os.listdir("tests/snapshots")):
    content = open(f"tests/snapshots/{f}").read().strip()
    print(f"  {f}: {content}")
EOF

echo "Verifying snapshot tests pass..."
cargo test --test snapshot_tests
