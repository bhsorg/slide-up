/** Status of the offline SenseVoice caption model. */
export interface ModelStatus {
  isDownloaded: boolean;
  isLoaded: boolean;
  activeProvider: string;
  modelSizeMb: number;
}

/** Throttled payload of the `model-download-progress` Tauri event. */
export interface DownloadProgress {
  percentage: number;
  downloadedMb: number;
  totalMb: number;
  status: string;
}

/** A downloadable AI upscaler model known to the app. */
export interface UpscalerModelStatus {
  id: string;
  name: string;
  fileName: string;
  sizeMb: number;
  speed: string;
  description: string;
  /** `"model"` for ONNX files, `"shader"` for mpv GLSL shaders. */
  kind: string;
  isDownloaded: boolean;
  isAvailable: boolean;
  sizeOnDiskMb: number;
}

/** Throttled payload of the `upscale-progress` Tauri event. */
export interface UpscaleProgress {
  outputPath: string;
  processedFrames: number;
  totalFrames: number;
  percentage: number;
  status: string;
}
