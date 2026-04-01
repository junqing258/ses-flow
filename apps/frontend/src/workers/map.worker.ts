import type { ExportFormat, ExportPayload, MapOverviewStats, MapProject, PlatformPanel } from "@/types/map";

type WorkerRequest = {
  requestId: number;
  type: "stats" | "export" | "plan-panels";
  payload: {
    project: MapProject;
    format?: ExportFormat;
  };
};

type WorkerResponse = {
  requestId: number;
  ok: boolean;
  result?: MapOverviewStats | ExportPayload | PlatformPanel[];
  error?: string;
};

type WorkerScope = {
  onmessage: ((event: { data: WorkerRequest }) => void) | null;
  postMessage: (message: WorkerResponse) => void;
};

const workerScope = self as unknown as WorkerScope;

const createStats = (project: MapProject): MapOverviewStats => ({
  deviceCount: project.devices.length,
  robotPathCount: project.overlays.robotPaths.length,
  platformPanelCount: project.overlays.platformPanels.length,
});

const createExportPayload = (project: MapProject, format: ExportFormat = "json"): ExportPayload => ({
  content: format === "jsonl" ? `${JSON.stringify(project)}\n` : JSON.stringify(project, null, 2),
  filename: format === "jsonl" ? "map-project.jsonl" : "map-project.json",
  mimeType: format === "jsonl" ? "application/x-ndjson" : "application/json",
});

workerScope.onmessage = (event: { data: WorkerRequest }) => {
  const { requestId, type, payload } = event.data;

  try {
    let result: MapOverviewStats | ExportPayload | PlatformPanel[];

    if (type === "stats") {
      result = createStats(payload.project);
    } else if (type === "export") {
      result = createExportPayload(payload.project, payload.format);
    } else {
      result = payload.project.overlays.platformPanels;
    }

    const response: WorkerResponse = {
      requestId,
      ok: true,
      result,
    };

    workerScope.postMessage(response);
  } catch (error) {
    const response: WorkerResponse = {
      requestId,
      ok: false,
      error: error instanceof Error ? error.message : "Unknown worker error",
    };

    workerScope.postMessage(response);
  }
};

export {};
