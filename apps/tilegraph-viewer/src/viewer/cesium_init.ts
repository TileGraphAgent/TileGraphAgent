import * as Cesium from "cesium";
import "cesium/Build/Cesium/Widgets/widgets.css";

export interface TileGraphViewer {
  viewer: Cesium.Viewer;
  tilesetRef: { tileset: Cesium.Cesium3DTileset | null };
  highlightObjects: (objectIds: string[], color?: Cesium.Color) => void;
  clearHighlights: () => void;
  isolateObjects: (objectIds: string[]) => void;
  focusCameraOn: (objectIds: string[]) => void;
  showBoundingBoxes: (objectIds: string[]) => void;
}

// Feature ID → object_id lookup (populated after tile load)
const featureIdToObjectId: Map<number, string> = new Map();
const objectIdToFeatureId: Map<string, number> = new Map();

export async function initCesiumViewer(
  containerId: string,
  tilesetPath: string,
  onObjectSelected: (objectId: string, tag: string | null) => void
): Promise<TileGraphViewer> {
  // Cesium Ion is not needed for local tiles
  Cesium.Ion.defaultAccessToken = "";

  const viewer = new Cesium.Viewer(containerId, {
    baseLayerPicker: false,
    geocoder: false,
    homeButton: false,
    infoBox: false,
    navigationHelpButton: false,
    sceneModePicker: false,
    selectionIndicator: false,
    timeline: false,
    animation: false,
    scene3DOnly: true,
    skyBox: false,
    skyAtmosphere: false,
    baseLayer: false,
  });

  viewer.scene.backgroundColor = new Cesium.Color(0.12, 0.12, 0.16, 1.0);

  // Load 3D Tiles
  let tilesetObj: Cesium.Cesium3DTileset | null = null;
  const tilesetRef = { tileset: null as Cesium.Cesium3DTileset | null };

  try {
    const tileset = await Cesium.Cesium3DTileset.fromUrl(tilesetPath);
    viewer.scene.primitives.add(tileset);
    await viewer.zoomTo(tileset);
    tilesetObj = tileset;
    tilesetRef.tileset = tileset;

    // Default style
    tileset.style = new Cesium.Cesium3DTileStyle({
      color: "color('white', 0.9)",
    });
  } catch (err) {
    console.error("Failed to load tileset:", err);
  }

  // Object selection via feature picking
  viewer.screenSpaceEventHandler.setInputAction(
    (movement: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      const picked = viewer.scene.pick(movement.position);
      if (!picked || !Cesium.defined(picked)) return;

      const feature = picked as Cesium.Cesium3DTileFeature;
      if (feature instanceof Cesium.Cesium3DTileFeature) {
        const objectId = feature.getProperty("object_id") as string | undefined;
        const tag = feature.getProperty("tag") as string | undefined;
        if (objectId) {
          onObjectSelected(objectId, tag ?? null);
        }
      }
    },
    Cesium.ScreenSpaceEventType.LEFT_CLICK
  );

  // --- Highlight / isolation helpers ---
  const defaultColor = new Cesium.Color(0.8, 0.8, 0.8, 0.9);
  const highlightColor = new Cesium.Color(0.0, 0.8, 1.0, 1.0);
  const isolationHideColor = new Cesium.Color(0.2, 0.2, 0.2, 0.1);

  const highlightObjects = (objectIds: string[], color?: Cesium.Color): void => {
    if (!tilesetRef.tileset) return;
    const idSet = new Set(objectIds);
    tilesetRef.tileset.style = new Cesium.Cesium3DTileStyle({
      color: {
        conditions: [
          // Highlighted objects get the agent color
          ["Boolean(${object_id}) === true && true", `color('cyan', 1.0)`],
          ["true", `color('white', 0.7)`],
        ],
      },
    });
    // Better: use per-feature color via Cesium3DTileStyle with conditions on object_id
    // Full implementation would iterate features when tile loads and set colors individually
  };

  const clearHighlights = (): void => {
    if (!tilesetRef.tileset) return;
    tilesetRef.tileset.style = new Cesium.Cesium3DTileStyle({
      color: "color('white', 0.9)",
    });
  };

  const isolateObjects = (objectIds: string[]): void => {
    if (!tilesetRef.tileset) return;
    const idList = objectIds.map((id) => `'${id}'`).join(",");
    tilesetRef.tileset.style = new Cesium.Cesium3DTileStyle({
      show: `[${idList}].indexOf(String(\${object_id})) >= 0`,
      color: "color('cyan', 1.0)",
    });
  };

  const focusCameraOn = (objectIds: string[]): void => {
    if (!tilesetRef.tileset) return;
    viewer.zoomTo(tilesetRef.tileset);
  };

  const showBoundingBoxes = (_objectIds: string[]): void => {
    if (!tilesetRef.tileset) return;
    tilesetRef.tileset.debugShowBoundingVolume = true;
  };

  return {
    viewer,
    tilesetRef,
    highlightObjects,
    clearHighlights,
    isolateObjects,
    focusCameraOn,
    showBoundingBoxes,
  };
}
