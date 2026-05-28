import { readFile } from "fs/promises";
import { existsSync } from "fs";

export interface SpatialRecord {
  object_id: string;
  tag: string | null;
  class: string;
  aabb_min: [number, number, number];
  aabb_max: [number, number, number];
  tile_id: string | null;
  feature_id: number | null;
}

interface SerializedIndex {
  version: string;
  record_count: number;
  records: SpatialRecord[];
}

export class SpatialIndexClient {
  private records: SpatialRecord[] = [];
  private path: string;

  constructor(path: string) {
    this.path = path;
  }

  async load(): Promise<void> {
    if (!existsSync(this.path)) {
      console.error(`[SpatialIndex] File not found: ${this.path} — spatial queries will be empty`);
      return;
    }
    const raw = await readFile(this.path, "utf-8");
    const data: SerializedIndex = JSON.parse(raw);
    this.records = data.records;
    console.error(`[SpatialIndex] Loaded ${this.records.length} records`);
  }

  center(rec: SpatialRecord): [number, number, number] {
    return [
      (rec.aabb_min[0] + rec.aabb_max[0]) / 2,
      (rec.aabb_min[1] + rec.aabb_max[1]) / 2,
      (rec.aabb_min[2] + rec.aabb_max[2]) / 2,
    ];
  }

  distance(rec: SpatialRecord, point: [number, number, number]): number {
    const c = this.center(rec);
    const dx = c[0] - point[0];
    const dy = c[1] - point[1];
    const dz = c[2] - point[2];
    return Math.sqrt(dx * dx + dy * dy + dz * dz);
  }

  queryNearby(
    center: [number, number, number],
    radiusM: number,
    classFilter?: string
  ): Array<SpatialRecord & { distance_m: number }> {
    return this.records
      .filter((r) => {
        if (classFilter && r.class !== classFilter) return false;
        return this.distance(r, center) <= radiusM;
      })
      .map((r) => ({ ...r, distance_m: this.distance(r, center) }))
      .sort((a, b) => a.distance_m - b.distance_m);
  }

  findByObjectId(objectId: string): SpatialRecord | undefined {
    return this.records.find((r) => r.object_id === objectId);
  }

  findByTag(tag: string): SpatialRecord | undefined {
    return this.records.find((r) => r.tag === tag);
  }

  get count(): number {
    return this.records.length;
  }
}
