import type { MediaItem } from "../types/media";

export interface ParsedM3uResult {
  title: string;
  items: MediaItem[];
  audioCount: number;
  videoCount: number;
}

export function normalizeMediaUrl(rawPath: string, baseUrl?: string): string {
  let url = rawPath.trim();
  if (!url) return "";

  // If local file path without baseUrl, return as-is
  if (!/^https?:\/\/|^rt[ms]p:\/\//i.test(url) && !baseUrl) {
    return url;
  }

  // Resolve relative URL against baseUrl directory
  if (baseUrl && !/^https?:\/\/|^rt[ms]p:\/\//i.test(url)) {
    try {
      const baseDir = baseUrl.replace(/\/[^/]+\.[^/]+$/, "/");
      url = new URL(url, baseDir).href;
    } catch {
      // fallback
    }
  }

  // Upgrade http to https for web streams (Archive.org requires https)
  if (url.startsWith("http://")) {
    url = "https://" + url.slice(7);
  }

  // Percent-encode pathname segments so spaces (%20) and ampersands (%26) do not break HTTP requests
  try {
    const parsed = new URL(url);
    const encodedPath = parsed.pathname
      .split("/")
      .map((segment) => {
        try {
          return encodeURIComponent(decodeURIComponent(segment));
        } catch {
          return encodeURIComponent(segment);
        }
      })
      .join("/");

    parsed.pathname = encodedPath;
    return parsed.href;
  } catch {
    return url.replace(/\s+/g, "%20").replace(/&/g, "%26");
  }
}

export function parseM3u(content: string, defaultTitle = "Imported Playlist", baseUrl?: string): ParsedM3uResult {
  const lines = content.split(/\r?\n/);
  const items: MediaItem[] = [];

  let playlistTitle = defaultTitle;
  let currentDuration = 0;
  let currentTitle = "";
  let currentArtist = "";
  let currentLogo = "";
  let currentGroup = "";
  let isRadio = false;

  const audioExts = new Set(["mp3", "m4a", "aac", "flac", "ogg", "oga", "opus", "wav", "wma", "alac", "aiff", "aif", "mid", "midi"]);
  const videoExts = new Set(["mp4", "mkv", "avi", "mov", "webm", "flv", "ts", "m2ts", "mpg", "mpeg", "3gp", "wmv", "asf"]);

  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const line = rawLine.trim();
    if (!line) continue;

    if (line.startsWith("#PLAYLIST:")) {
      const parsedName = line.substring(10).trim();
      if (parsedName) playlistTitle = parsedName;
      continue;
    }

    if (line.startsWith("#EXTINF:")) {
      const rest = line.substring(8).trim();
      const commaIdx = rest.lastIndexOf(",");
      let rawTitle = commaIdx !== -1 ? rest.substring(commaIdx + 1).trim() : "";
      const meta = commaIdx !== -1 ? rest.substring(0, commaIdx) : rest;

      const durationMatch = meta.match(/^-?\d+/);
      currentDuration = durationMatch ? Math.max(0, parseInt(durationMatch[0], 10)) * 1000 : 0;

      const getAttr = (attr: string): string => {
        const match = meta.match(new RegExp(`${attr}="([^"]*)"`, "i"));
        return match ? match[1].trim() : "";
      };

      const tvgName = getAttr("tvg-name");
      currentLogo = getAttr("tvg-logo");
      currentGroup = getAttr("group-title");
      isRadio = /radio="?true"?|radio="?1"?/i.test(meta) || /radio|audio|podcast/i.test(currentGroup);

      if (!rawTitle && tvgName) {
        rawTitle = tvgName;
      }

      if (rawTitle.includes(" - ")) {
        const parts = rawTitle.split(" - ");
        currentArtist = parts[0].trim();
        currentTitle = parts.slice(1).join(" - ").trim();
      } else {
        currentTitle = rawTitle || "Track";
        currentArtist = currentGroup || (isRadio ? "Radio" : "");
      }
    } else if (!line.startsWith("#")) {
      const path = normalizeMediaUrl(line, baseUrl);
      if (!path) continue;

      const cleanPath = path.split("?")[0];
      const extMatch = cleanPath.match(/\.([a-z0-9]+)$/i);
      const ext = extMatch ? extMatch[1].toLowerCase() : "";

      // Determine Audio vs Video (Live TV):
      let kind: "audio" | "video" = "audio";
      if (isRadio || audioExts.has(ext)) {
        kind = "audio";
      } else if (videoExts.has(ext)) {
        kind = "video";
      } else if (/tv|live|sports|news|movies|cinema|series|entertainment|documentary|kids/i.test(currentGroup)) {
        kind = "video";
      } else if (ext === "m3u8") {
        kind = isRadio ? "audio" : "video";
      } else {
        kind = "audio";
      }

      let title = currentTitle;
      if (!title || title === "Track") {
        try {
          const rawFilename = path.split("/").pop()?.split("?")[0] || "Track";
          title = decodeURIComponent(rawFilename).replace(/\.[^/.]+$/, "");
        } catch {
          title = "Track " + (items.length + 1);
        }
      }

      const artist = currentArtist || (kind === "video" ? (currentGroup || "Live TV") : "Audio Stream");

      items.push({
        id: -Math.floor(Date.now() % 1000000000 + items.length + 1),
        path,
        kind,
        title,
        artist: artist || null,
        album: currentGroup || (kind === "video" ? "Live TV" : playlistTitle),
        durationMs: currentDuration,
        codec: ext ? ext.toUpperCase() : (kind === "video" ? "Live Stream" : "Audio"),
        width: null,
        height: null,
        sizeBytes: 0,
        thumbnailPath: currentLogo || null,
        favorite: false,
        playCount: 0,
        lastPlayedAt: null,
        addedAt: Date.now(),
      });

      currentDuration = 0;
      currentTitle = "";
      currentArtist = "";
      currentLogo = "";
      currentGroup = "";
      isRadio = false;
    }
  }

  return {
    title: playlistTitle,
    items,
    audioCount: items.filter((i) => i.kind === "audio").length,
    videoCount: items.filter((i) => i.kind === "video").length,
  };
}
