# AI-Assisted Development Verification Log

## Policy

Every AI-generated code block in this project is:
1. Reviewed for correctness before commit
2. Tested against a real execution path (not just type-checked)
3. Verified against official specifications (3D Tiles, glTF, Neo4j, MCP)
4. Noted in this document with the verification method used

---

## Verification Checklist per Crate

### tilegraph-core
- [ ] ObjectId determinism test: same source → same ID across runs
- [ ] AABB union correctness: validated with known min/max values
- [ ] Transform matrix composition: checked against glam test vectors
- [ ] 3D Tiles box format: validated against spec [cx, cy, cz, hx, 0, 0, 0, hy, 0, 0, 0, hz]

### tilegraph-synth
- [ ] Unique tag constraint: `tilegraph validate` reports 0 duplicate tags
- [ ] All objects with geometry have valid AABB (is_valid == true)
- [ ] Parent/child hierarchy forms a tree (no cycles)
- [ ] JSON output matches plant_spec.json structure

### tilegraph-geometry
- [ ] Cylinder mesh: 0 degenerate triangles (no NaN positions/normals)
- [ ] Box mesh: exactly 12 triangles, 24 vertices
- [ ] Feature IDs are assigned sequentially without collisions within a batch
- [ ] World AABB computed from actual vertex positions

### tilegraph-gltf
- [ ] GLB magic bytes: `glTF` at byte offset 0
- [ ] GLB version: 0x02 at bytes 4–7
- [ ] JSON chunk type: `JSON` at chunk header
- [ ] BIN chunk type: `BIN\0` at chunk header
- [ ] All buffer views point within valid buffer range
- [ ] Accessor count matches primitive vertex count
- [ ] `_FEATURE_ID_0` attribute is SCALAR UNSIGNED_INT, one value per vertex
- [ ] Loaded in CesiumJS without console errors

### tilegraph-tiles
- [ ] tileset.json validates against 3D Tiles 1.1 spec
- [ ] asset.version = "1.1"
- [ ] Root geometric error >= any leaf geometric error
- [ ] All content URIs point to existing .glb files
- [ ] bounding volume box has non-negative half-extents

### tilegraph-spatial
- [ ] R-tree query returns correct subset for known test case
- [ ] Nearby query: P-1001 at (0,0,0) with radius 1m returns only P-1001
- [ ] Serialized index loads correctly and produces same query results

### tilegraph-graph-export
- [ ] Cypher MERGE statements are idempotent (safe to re-run)
- [ ] CSV node headers match neo4j-admin import format
- [ ] No orphan relationships (all source/target IDs exist in nodes)
- [ ] Schema init script runs without errors on empty Neo4j 5.x instance

### tilegraph-cli
- [ ] `tilegraph generate-synth` exits 0 with expected output
- [ ] `tilegraph build-tiles` produces tileset.json and *.glb files
- [ ] `tilegraph build-graph` produces nodes.csv and relationships.csv
- [ ] `tilegraph validate` reports PASSED on clean run

---

## AI Tool Prompts Used (examples)

### Implementing a Rust crate
```
Context: I am building the tilegraph-core crate for an industrial 3D pipeline in Rust.
Task: Implement the Aabb struct with union(), expand_by_point(), to_3dtiles_box() methods.
Constraint: to_3dtiles_box() must return [cx, cy, cz, hx, 0, 0, 0, hy, 0, 0, 0, hz] exactly matching the 3D Tiles 1.1 spec section 4.3.
Test: Write a unit test that verifies a box from (0,0,0) to (2,2,2) produces center=(1,1,1) and half_extents=(1,1,1).
```

### Generating Neo4j Cypher
```
Context: I have an EngObject graph where:
- Pump has :CONNECTED_TO relationship to Line
- Valve has :PART_OF and :ISOLATED_BY relationships to Line
Task: Write a Cypher query that given a Line tag, returns all connected pumps AND their isolation valves.
Constraint: Must handle Lines with no pumps (return empty list, not error).
Verify: Test against the example graph where LINE-1001 has 1 pump (P-1001) and 4 valves.
```

### Reviewing unsafe assumptions
```
I generated this Rust code that assumes an Option<Aabb> is always Some() for geometry objects.
Review: Is this safe? What objects in the IndustrialObject model might have class.has_geometry() == true but no AABB?
Answer: Describe the invariant and where I should add an explicit check.
```

### Checking 3D Tiles schema
```
I have this tileset.json snippet. Verify it conforms to 3D Tiles 1.1 spec:
- Check asset.version is "1.1"
- Check geometricError is positive and decreases from root to leaf
- Check bounding volume box has 12 values
- Check refine is "ADD" or "REPLACE"
Reference: https://docs.ogc.org/cs/22-025r4/22-025r4.html
```

---

## AI-Assisted Commit Convention

Every commit that includes AI-generated code must include:
```
[AI-assisted] <description>

AI role: [architecture|implementation|test generation|debugging|review]
Verified: [test name or manual verification description]
Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

---

## Known AI Error Patterns to Watch For

1. **Rust lifetime/borrow assumptions** — AI often generates code that borrows after move. Always check.
2. **glTF index confusion** — AI confuses bufferView indices and accessor indices. Cross-check counts.
3. **Cypher parameter types** — AI uses string params where Neo4j expects integers. Check types.
4. **3D Tiles bounding box column-major confusion** — AI sometimes transposes the box matrix. Verify against spec.
5. **Feature ID collision** — AI-generated feature ID assignment may not be globally unique across batches. Verify monotonic increment.
