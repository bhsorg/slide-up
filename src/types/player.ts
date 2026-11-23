export type PlayMode = "off" | "all" | "one";
export type PlayerStatus = "stopped" | "loading" | "playing" | "paused" | "error";

export interface TrackInfo {
  id: number;
  path: string;
  title: string;
  artist: string | null;
  album: string | null;
  kind: "audio" | "video";
  thumbnailPath: string | null;
  durationMs: number;
  codec?: string | null;
}

export interface PlayerState {
  status: PlayerStatus;
  current: TrackInfo | null;
  positionMs: number;
  durationMs: number;
  volume: number;
  muted: boolean;
  speed: number;
  shuffle: boolean;
  repeat: PlayMode;
  fullscreen: boolean;
  videoTracks: MediaTrack[];
  audioTracks: MediaTrack[];
  subtitleTracks: MediaTrack[];
  activeVideoTrack: number;
  activeAudioTrack: number;
  activeSubtitleTrack: number;
  videoWidth: number;
  videoHeight: number;
  error: string | null;
}

export interface MediaTrack {
  id: number;
  name: string;
  language: string | null;
  selected: boolean;
}

export interface PlayerSnapshot {
  positionMs: number;
  durationMs: number;
  status: PlayerStatus;
  volume: number;
  muted: boolean;
  speed: number;
  videoWidth: number;
  videoHeight: number;
}

export const initialPlayerState = (): PlayerState => ({
  status: "stopped",
  current: null,
  positionMs: 0,
  durationMs: 0,
  volume: 0.8,
  muted: false,
  speed: 1,
  shuffle: false,
  repeat: "off",
  fullscreen: false,
  videoTracks: [],
  audioTracks: [],
  subtitleTracks: [],
  activeVideoTrack: -1,
  activeAudioTrack: -1,
  activeSubtitleTrack: -1,
  videoWidth: 0,
  videoHeight: 0,
  error: null,
});
