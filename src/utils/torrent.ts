import type { MediaItem } from "../types/media";
import { formatBytes } from "../types/media";
import { normalizeMediaUrl } from "./m3u";

export interface TorrentTrackItem extends MediaItem {
  ext: string;
  fileIndex: number;
  companionThumbnail?: string | null;
  formattedSize: string;
}

export interface ParsedTorrentResult {
  name: string;
  infoHash: string;
  magnetUri: string;
  comment?: string;
  totalSizeBytes: number;
  thumbnail: string | null;
  items: TorrentTrackItem[];
  allExtensions: string[];
  audioCount: number;
  videoCount: number;
  hasWebSeeds: boolean;
  webSeedBase?: string;
}

const AUDIO_EXTS = new Set(["mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wma", "alac", "aiff", "aif", "mid"]);
const VIDEO_EXTS = new Set(["mp4", "mkv", "avi", "mov", "webm", "flv", "ts", "m2ts", "mpg", "mpeg", "3gp", "wmv", "m4v", "asf"]);
const IMAGE_EXTS = new Set(["jpg", "jpeg", "png", "webp", "gif"]);

function bytesToBase64Image(bytes: Uint8Array): string | null {
  if (!bytes || bytes.length < 4) return null;
  let mime = "image/jpeg";
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) {
    mime = "image/png";
  } else if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) {
    mime = "image/gif";
  } else if (bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46) {
    mime = "image/webp";
  }
  let binary = "";
  const len = bytes.byteLength;
  for (let i = 0; i < len; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  try {
    return `data:${mime};base64,${btoa(binary)}`;
  } catch {
    return null;
  }
}

class BencodeDecoder {
  private data: Uint8Array;
  private pos = 0;
  public infoBytes: Uint8Array | null = null;

  constructor(buffer: Uint8Array) {
    this.data = buffer;
  }

  decode(): any {
    if (this.pos >= this.data.length) return null;
    const byte = this.data[this.pos];

    if (byte === 105 /* 'i' */) {
      return this.decodeInt();
    } else if (byte === 108 /* 'l' */) {
      return this.decodeList();
    } else if (byte === 100 /* 'd' */) {
      return this.decodeDict();
    } else if (byte >= 48 && byte <= 57 /* '0'-'9' */) {
      return this.decodeBytes();
    } else {
      throw new Error(`Invalid bencode byte ${byte} at offset ${this.pos}`);
    }
  }

  private decodeInt(): number {
    this.pos++; // skip 'i'
    const start = this.pos;
    while (this.pos < this.data.length && this.data[this.pos] !== 101 /* 'e' */) {
      this.pos++;
    }
    const str = new TextDecoder("ascii").decode(this.data.subarray(start, this.pos));
    this.pos++; // skip 'e'
    return parseInt(str, 10);
  }

  private decodeBytes(): Uint8Array {
    const start = this.pos;
    while (this.pos < this.data.length && this.data[this.pos] !== 58 /* ':' */) {
      this.pos++;
    }
    const lenStr = new TextDecoder("ascii").decode(this.data.subarray(start, this.pos));
    const len = parseInt(lenStr, 10);
    this.pos++; // skip ':'
    const bytes = this.data.subarray(this.pos, this.pos + len);
    this.pos += len;
    return bytes;
  }

  private decodeList(): any[] {
    this.pos++; // skip 'l'
    const list: any[] = [];
    while (this.pos < this.data.length && this.data[this.pos] !== 101 /* 'e' */) {
      list.push(this.decode());
    }
    this.pos++; // skip 'e'
    return list;
  }

  private decodeDict(): Record<string, any> {
    this.pos++; // skip 'd'
    const dict: Record<string, any> = {};
    const textDecoder = new TextDecoder("utf-8");

    while (this.pos < this.data.length && this.data[this.pos] !== 101 /* 'e' */) {
      const keyBytes = this.decodeBytes();
      const key = textDecoder.decode(keyBytes);

      if (key === "info") {
        const infoStart = this.pos;
        const val = this.decode();
        const infoEnd = this.pos;
        this.infoBytes = this.data.subarray(infoStart, infoEnd);
        dict[key] = val;
      } else {
        dict[key] = this.decode();
      }
    }
    this.pos++; // skip 'e'
    return dict;
  }
}

interface FileDescriptor {
  index: number;
  fullPath: string;
  dirPath: string;
  fileName: string;
  baseName: string;
  ext: string;
  length: number;
}

export async function parseTorrent(buffer: ArrayBuffer | Uint8Array): Promise<ParsedTorrentResult> {
  const bytes = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
  const decoder = new BencodeDecoder(bytes);
  const root = decoder.decode();

  if (!root || typeof root !== "object" || !root.info) {
    throw new Error("Invalid .torrent file: missing 'info' dictionary.");
  }

  const textDecoder = new TextDecoder("utf-8");
  const decodeString = (val: any): string => {
    if (!val) return "";
    if (typeof val === "string") return val;
    if (val instanceof Uint8Array) return textDecoder.decode(val);
    return String(val);
  };

  const info = root.info;
  const torrentName = decodeString(info["name.utf-8"] || info.name) || "Torrent Media";
  const comment = decodeString(root.comment);

  // Compute BitTorrent Info Hash (SHA-1 of info dictionary)
  let infoHash = "";
  if (decoder.infoBytes && crypto?.subtle) {
    try {
      const digest = await crypto.subtle.digest("SHA-1", decoder.infoBytes as any);
      infoHash = Array.from(new Uint8Array(digest))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
    } catch {
      // fallback
    }
  }

  // Extract embedded thumbnail if present in metadata
  let thumbnailDataUrl: string | null = null;
  const rawThumb = info.thumbnail || root.thumbnail || info.cover || root.cover;
  if (rawThumb instanceof Uint8Array) {
    thumbnailDataUrl = bytesToBase64Image(rawThumb);
  }

  // Build Magnet URI
  let magnetUri = `magnet:?xt=urn:btih:${infoHash || "unknown"}&dn=${encodeURIComponent(torrentName)}`;
  const announce = decodeString(root.announce);
  if (announce) {
    magnetUri += `&tr=${encodeURIComponent(announce)}`;
  }

  // 1. Extract Web Seed URLs (BEP 19 GetRight web seeds & Archive.org)
  const webSeeds: string[] = [];
  const rawUrlList = root["url-list"] || info["url-list"];
  if (typeof rawUrlList === "string") {
    webSeeds.push(rawUrlList);
  } else if (rawUrlList instanceof Uint8Array) {
    webSeeds.push(decodeString(rawUrlList));
  } else if (Array.isArray(rawUrlList)) {
    for (const u of rawUrlList) {
      const s = decodeString(u).trim();
      if (s) webSeeds.push(s);
    }
  }

  const rawHttpSeeds = root["httpseeds"] || info["httpseeds"];
  if (typeof rawHttpSeeds === "string") {
    webSeeds.push(rawHttpSeeds);
  } else if (Array.isArray(rawHttpSeeds)) {
    for (const u of rawHttpSeeds) {
      const s = decodeString(u).trim();
      if (s) webSeeds.push(s);
    }
  }

  // Detect Archive.org item identifier from comment or announce
  let archiveItemId = "";
  if (comment) {
    const m = comment.match(/archive\.org\/details\/([^\s\/\?\#]+)/i);
    if (m) archiveItemId = m[1];
  }
  if (!archiveItemId && announce && announce.includes("archive.org")) {
    archiveItemId = torrentName;
  }

  // Determine primary web seed base URL
  let primaryWebSeedBase = "";
  if (archiveItemId) {
    primaryWebSeedBase = `https://archive.org/download/${archiveItemId}/`;
  } else if (webSeeds.length > 0) {
    // Prefer https and archive.org/download/
    const preferred =
      webSeeds.find((w) => w.includes("archive.org/download/")) ||
      webSeeds.find((w) => w.startsWith("https://")) ||
      webSeeds[0];
    let base = preferred;
    if (base.startsWith("http://") && base.includes("archive.org")) {
      base = "https://" + base.slice(7);
    }
    // BEP 19: If multi-file torrent and base does not end with folder name, append torrentName
    if (Array.isArray(info.files) && !base.toLowerCase().includes(torrentName.toLowerCase())) {
      if (!base.endsWith("/")) base += "/";
      base += `${encodeURIComponent(torrentName)}/`;
    }
    if (!base.endsWith("/")) base += "/";
    primaryWebSeedBase = base;
  }

  // Parse raw file list
  const allFiles: FileDescriptor[] = [];
  let totalSizeBytes = 0;

  if (Array.isArray(info.files)) {
    for (let i = 0; i < info.files.length; i++) {
      const f = info.files[i];
      const length = typeof f.length === "number" ? f.length : 0;
      totalSizeBytes += length;

      const pathParts = Array.isArray(f["path.utf-8"] || f.path)
        ? (f["path.utf-8"] || f.path).map(decodeString)
        : [decodeString(f.path)];
      const fullPath = pathParts.join("/");
      const fileName = pathParts[pathParts.length - 1] || "Track";
      const dirPath = pathParts.slice(0, -1).join("/");

      const extMatch = fileName.match(/\.([a-z0-9]+)$/i);
      const ext = extMatch ? extMatch[1].toLowerCase() : "";
      const baseName = fileName.replace(/\.[^/.]+$/, "");

      allFiles.push({
        index: i,
        fullPath,
        dirPath,
        fileName,
        baseName,
        ext,
        length,
      });
    }
  } else {
    // Single-file torrent
    const length = typeof info.length === "number" ? info.length : 0;
    totalSizeBytes = length;
    const fileName = torrentName;
    const extMatch = fileName.match(/\.([a-z0-9]+)$/i);
    const ext = extMatch ? extMatch[1].toLowerCase() : "";
    const baseName = fileName.replace(/\.[^/.]+$/, "");

    allFiles.push({
      index: 0,
      fullPath: fileName,
      dirPath: "",
      fileName,
      baseName,
      ext,
      length,
    });
  }

  // 1. Build Companion and Folder image index for smart artwork matching
  // Matches: "filename.mp3" <-> "filename.jpg" or "filename.png"
  const exactImageMap = new Map<string, string>();
  // Matches: "cover.jpg", "folder.jpg", "poster.png", "front.jpg", "albumart.jpg"
  const folderArtworkMap = new Map<string, string>();

  for (const file of allFiles) {
    if (IMAGE_EXTS.has(file.ext)) {
      const key = `${file.dirPath}/${file.baseName.toLowerCase()}`;
      exactImageMap.set(key, file.fileName);

      if (/cover|folder|poster|front|albumart|thumb/i.test(file.fileName)) {
        if (!folderArtworkMap.has(file.dirPath)) {
          folderArtworkMap.set(file.dirPath, file.fileName);
        }
      }
    }
  }

  // Determine torrent-level cover art if available from web seeds
  let folderArtworkUrl: string | null = null;
  const mainFolderArt = folderArtworkMap.get("") || Array.from(folderArtworkMap.values())[0];
  if (mainFolderArt && primaryWebSeedBase) {
    folderArtworkUrl = normalizeMediaUrl(`${primaryWebSeedBase}${encodeURIComponent(mainFolderArt)}`);
  }
  const effectiveTorrentThumbnail = folderArtworkUrl || thumbnailDataUrl;

  // 2. Filter & format media items
  const items: TorrentTrackItem[] = [];
  const extensionsSet = new Set<string>();

  for (const file of allFiles) {
    const isAudio = AUDIO_EXTS.has(file.ext);
    const isVideo = VIDEO_EXTS.has(file.ext);

    if (!isAudio && !isVideo) {
      continue; // Skip non-media files
    }

    extensionsSet.add(file.ext);
    const kind: "audio" | "video" = isAudio ? "audio" : "video";

    // Check for companion image (same name.png/jpg or folder artwork)
    const exactKey = `${file.dirPath}/${file.baseName.toLowerCase()}`;
    const companionThumbnailName =
      exactImageMap.get(exactKey) ||
      folderArtworkMap.get(file.dirPath) ||
      folderArtworkMap.get("") ||
      null;

    let companionThumbUrl: string | null = null;
    let trackPath = "";

    if (primaryWebSeedBase) {
      // Build direct HTTP(S) stream URL for this track
      let relativePath = file.fullPath;
      if (archiveItemId && relativePath.toLowerCase().startsWith(`${archiveItemId.toLowerCase()}/`)) {
        relativePath = relativePath.slice(archiveItemId.length + 1);
      } else if (relativePath.toLowerCase().startsWith(`${torrentName.toLowerCase()}/`)) {
        relativePath = relativePath.slice(torrentName.length + 1);
      }

      const encodedPath = relativePath
        .split("/")
        .map((part) => encodeURIComponent(part))
        .join("/");

      trackPath = normalizeMediaUrl(`${primaryWebSeedBase}${encodedPath}`);

      if (companionThumbnailName) {
        let relativeThumb = companionThumbnailName;
        if (file.dirPath) {
          relativeThumb = `${file.dirPath}/${companionThumbnailName}`;
        }
        if (archiveItemId && relativeThumb.toLowerCase().startsWith(`${archiveItemId.toLowerCase()}/`)) {
          relativeThumb = relativeThumb.slice(archiveItemId.length + 1);
        } else if (relativeThumb.toLowerCase().startsWith(`${torrentName.toLowerCase()}/`)) {
          relativeThumb = relativeThumb.slice(torrentName.length + 1);
        }
        const encodedThumb = relativeThumb
          .split("/")
          .map((part) => encodeURIComponent(part))
          .join("/");
        companionThumbUrl = normalizeMediaUrl(`${primaryWebSeedBase}${encodedThumb}`);
      }
    } else {
      // Pure P2P torrent without web seeds
      trackPath = `torrent://${infoHash || "media"}/${encodeURIComponent(file.fullPath)}`;
    }

    let title = file.baseName;
    let artist: string | null = torrentName;
    if (file.baseName.includes(" - ")) {
      const parts = file.baseName.split(" - ");
      artist = parts[0].trim();
      title = parts.slice(1).join(" - ").trim();
    }

    items.push({
      id: -Math.floor(Date.now() % 1000000000 + items.length + 1),
      path: trackPath,
      kind,
      title,
      artist,
      album: torrentName,
      durationMs: 0,
      codec: file.ext.toUpperCase(),
      width: null,
      height: null,
      sizeBytes: file.length,
      thumbnailPath: companionThumbUrl || effectiveTorrentThumbnail,
      favorite: false,
      playCount: 0,
      lastPlayedAt: null,
      addedAt: Date.now(),
      ext: file.ext,
      fileIndex: file.index,
      companionThumbnail: companionThumbnailName,
      formattedSize: formatBytes(file.length),
    });
  }

  const audioCount = items.filter((i) => i.kind === "audio").length;
  const videoCount = items.filter((i) => i.kind === "video").length;
  const allExtensions = Array.from(extensionsSet).sort();

  return {
    name: torrentName,
    infoHash,
    magnetUri,
    comment,
    totalSizeBytes,
    thumbnail: effectiveTorrentThumbnail,
    items,
    allExtensions,
    audioCount,
    videoCount,
    hasWebSeeds: !!primaryWebSeedBase,
    webSeedBase: primaryWebSeedBase || undefined,
  };
}
