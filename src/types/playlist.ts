import type { MediaItem } from "./media";

export interface Playlist {
  id: number;
  name: string;
  createdAt: number;
  updatedAt: number;
  trackCount: number;
  coverPath: string | null;
}

export interface PlaylistTrack {
  media: MediaItem;
  position: number;
}

export interface QueueItem {
  media: MediaItem;
  source: "playlist" | "library" | "file" | "manual";
}
