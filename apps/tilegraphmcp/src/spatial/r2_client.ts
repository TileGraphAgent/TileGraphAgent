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

const R2_KEY = "tiles/index/spatial_index.json";

export class R2SpatialIndexClient {
  private records: SpatialRecord[] = [];
  private bucket: R2Bucket;

  constructor(bucket: R2Bucket) {
    this.bucket = bucket;
  }

  // Call once at worker startup before serving requests
  async load(): Promise<void> {
    const obj = await this.bucket.get(R2_KEY);
    if (!obj) {
      console.error(`[SpatialIndex] R2 key not found: ${R2_KEY}`);
      return;
    }
    const text = await obj.text();
    const data: SerializedIndex = JSON.parse(text);
    this.records = data.records;
    console.error(`[SpatialIndex] Loaded ${this.records.length} records from R2`);
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
    classFilter?: string,
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
