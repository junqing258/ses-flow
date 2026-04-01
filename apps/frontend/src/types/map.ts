export type ExportFormat = "json" | "jsonl";

export type MapScene = "production" | "simulation";

export interface PlatformPanel {
  id: string;
  name?: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  [key: string]: unknown;
}

export interface MapProject {
  grid: {
    width: number;
    height: number;
  };
  devices: Array<Record<string, unknown>>;
  overlays: {
    robotPaths: Array<Record<string, unknown>>;
    platformPanels: PlatformPanel[];
    [key: string]: unknown;
  };
  [key: string]: unknown;
}

export interface MapOverviewStats {
  deviceCount: number;
  robotPathCount: number;
  platformPanelCount: number;
}

export interface ExportPayload {
  content: string;
  filename: string;
  mimeType: string;
}

export interface MapLibraryItem {
  id: string;
  name: string;
  draft: boolean;
  scene: MapScene;
  tags: string[];
  updatedAt: string;
  project: MapProject;
}
