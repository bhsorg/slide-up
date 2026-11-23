import type { LyricsResult, LyricLine, LrclibRecord, LyricsSource } from "../types/lyrics";

const BASE_URL = "https://lrclib.net";
// Browsers forbid setting User-Agent, so LRCLIB's documented alternatives are used.
const CLIENT_ID = "Slide Up v0.1.0 (https://github.com/slideup)";

/** Parse an LRC string into time-ordered lines. Supports [mm:ss.xx] and [mm:ss.xxx]. */
export function parseLrc(text: string): LyricLine[] {
  const pattern = /^\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]\s*(.*)$/;
  const lines: LyricLine[] = [];

  for (const raw of text.split(/\r?\n/)) {
    const match = pattern.exec(raw.trim());
    if (!match) continue;
    const minutes = Number(match[1]);
    const seconds = Number(match[2]);
    const fraction = match[3] ?? "";
    const fractionMs = fraction.length === 2
      ? Number(fraction) * 10
      : fraction.length === 3
        ? Number(fraction)
        : 0;
    lines.push({
      timeMs: (minutes * 60 + seconds) * 1000 + fractionMs,
      text: match[4].trimEnd(),
    });
  }

  return lines.sort((a, b) => a.timeMs - b.timeMs);
}

/** Plain lyrics have no timings; they are shown as a static, scrollable block. */
export function parsePlainLyrics(text: string): LyricLine[] {
  return text.split(/\r?\n/).map((line) => ({ timeMs: -1, text: line.trimEnd() }));
}

async function getJson(path: string, params: Record<string, string | number | undefined>): Promise<unknown> {
  const url = new URL(BASE_URL + path);
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      url.searchParams.set(key, String(value));
    }
  }

  let response: Response;
  try {
    response = await fetch(url.toString(), {
      headers: {
        "Lrclib-Client": CLIENT_ID,
        "X-User-Agent": CLIENT_ID,
        Accept: "application/json",
      },
    });
  } catch (err) {
    console.warn("[Lyrics] Network error connecting to LRCLIB:", err);
    throw new Error("Could not reach the lyrics service. Check your internet connection.");
  }

  if (response.status === 404) return null;
  if (response.status === 429 || response.status === 503) {
    const retryAfter = Number(response.headers.get("Retry-After") ?? "1");
    throw new Error(`Lyrics service is busy; try again in ${Number.isFinite(retryAfter) ? retryAfter : 1} seconds.`);
  }
  if (!response.ok) {
    console.warn(`[Lyrics] HTTP ${response.status} from LRCLIB:`, await response.text().catch(() => ""));
    throw new Error(`Lyrics service returned an error (${response.status}).`);
  }
  return response.json();
}

function asRecord(value: unknown): LrclibRecord | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Partial<LrclibRecord>;
  if (typeof record.id !== "number" || typeof record.trackName !== "string") return null;
  return record as LrclibRecord;
}

function asRecordArray(value: unknown): LrclibRecord[] {
  return Array.isArray(value)
    ? value.map(asRecord).filter((record): record is LrclibRecord => record !== null)
    : [];
}

function toResult(record: LrclibRecord): LyricsResult | null {
  if (record.instrumental) return null;
  const synced = record.syncedLyrics ? parseLrc(record.syncedLyrics) : [];
  if (synced.length) {
    return { lines: synced, synced: true, trackName: record.trackName, artistName: record.artistName };
  }
  if (record.plainLyrics?.trim()) {
    return {
      lines: parsePlainLyrics(record.plainLyrics),
      synced: false,
      trackName: record.trackName,
      artistName: record.artistName,
    };
  }
  return null;
}

function closestByDuration(records: LrclibRecord[], durationSec: number | undefined): LrclibRecord | undefined {
  if (!records.length) return undefined;
  if (durationSec === undefined) return records[0];
  return records.reduce((best, record) =>
    Math.abs(record.duration - durationSec) < Math.abs(best.duration - durationSec) ? record : best,
  );
}

/** Clean audio titles: remove file extensions, track numbers, YouTube credits, and video/audio tags. */
function cleanTrackTitle(title: string): string {
  let cleaned = title.trim();

  // Strip pipe separator and trailing metadata (e.g. "| Emraan Hashmi, Vidya Balan" or "| T-Series")
  cleaned = cleaned.replace(/\s*\|.*$/, "");

  // Strip double slash separator (e.g. "// Official Video")
  cleaned = cleaned.replace(/\s*\/\/.*$/, "");

  // Strip common media extensions
  cleaned = cleaned.replace(/\.(mp3|flac|m4a|wav|aac|ogg|opus|mp4|mkv|webm)$/i, "");

  // Strip leading track numbers like "01. ", "01 - ", "01 "
  cleaned = cleaned.replace(/^\s*\d{1,3}[\s._\-]+\s*/, "");

  // Strip bracketed descriptors like (Official Video), [Lyrics], (Audio), (Remastered), (Full Song), [1080p]
  cleaned = cleaned.replace(
    /\s*[\(\[]\s*(official\s*(music\s*)?video|official\s*audio|lyrics?|visualizer|audio|remastered|lyric\s*video|hd|4k|320kbps|128kbps|flac|mp3|with\s+lyrics?|lyrics?\s+video|full\s*(video|audio)?\s*song|video\s*song|audio\s*song|full\s*song)\s*[\)\]]/gi,
    ""
  );

  // Strip cast/credit parentheticals like (starring ...), (featuring ...), [starring ...]
  cleaned = cleaned.replace(
    /\s*[\(\[]\s*(starring|featuring|sung\s+by|composed\s+by|music\s+by|starring\s*:)[^\)\]]*[\)\]]/gi,
    ""
  );

  // Strip trailing promo phrases like "- full song", "- full audio", "- lyrical video", etc.
  cleaned = cleaned.replace(
    /\s*[-–—]\s*(full\s*(video|audio)?\s*song|video\s*song|audio\s*song|full\s*video|full\s*audio|full\s*song|official\s*song|lyrical\s*song|lyric\s*video|audio|video)\s*$/gi,
    ""
  );

  // Strip standalone promo keywords: "full song", "video song", "audio song"
  cleaned = cleaned.replace(
    /\b(full\s*(video|audio)?\s*song|video\s*song|audio\s*song|full\s*song)\b/gi,
    ""
  );

  // Strip surrounding quotes
  cleaned = cleaned.replace(/^["'“”‘’]+|["'“”‘’]+$/g, "").trim();

  // Clean extra spaces and dangling hyphens
  cleaned = cleaned.replace(/\s{2,}/g, " ").replace(/\s*[-–—]\s*$/, "").trim();

  return cleaned;
}

/** Extract primary artist and track name from messy titles */
function extractArtistAndTrack(
  cleanedTitle: string,
  existingArtist?: string | null
): { trackName: string; artistName: string } {
  let artistName = (existingArtist || "").trim();
  let trackName = cleanedTitle;

  const isGenericArtist = !artistName || /unknown|various|network stream|stream|torrent/i.test(artistName);

  if (cleanedTitle.includes(" - ") || cleanedTitle.includes(" – ")) {
    const parts = cleanedTitle
      .split(/\s+[-–—]\s+/)
      .map((p) => cleanTrackTitle(p))
      .filter(Boolean);

    if (parts.length === 2) {
      if (isGenericArtist) {
        artistName = parts[0];
        trackName = parts[1];
      } else {
        if (
          parts[0].toLowerCase().includes(artistName.toLowerCase()) ||
          artistName.toLowerCase().includes(parts[0].toLowerCase())
        ) {
          artistName = parts[0];
          trackName = parts[1];
        } else if (parts[1].toLowerCase().includes(artistName.toLowerCase())) {
          artistName = parts[1];
          trackName = parts[0];
        } else {
          trackName = parts[1];
        }
      }
    } else if (parts.length >= 3) {
      if (isGenericArtist) {
        artistName = parts[0];
        trackName = parts[1];
      } else {
        trackName = parts[1] || parts[0];
      }
    }
  }

  return {
    trackName: cleanTrackTitle(trackName),
    artistName: cleanTrackTitle(artistName),
  };
}

/** Generate spelling / transliteration variants (e.g. "Humari" <-> "Hamari", "Adhoori" <-> "Adhuri") */
function generateQueryCandidates(
  track: string,
  artist: string
): Array<{ track: string; artist: string; q: string }> {
  const list: Array<{ track: string; artist: string; q: string }> = [];
  const tracks = new Set<string>();
  tracks.add(track);

  // Variant 1: "hum" <-> "ham" (Humari <-> Hamari, Hum <-> Ham)
  if (/\bhum(ari|are|ara|e|ein)?\b/i.test(track)) {
    tracks.add(track.replace(/\bhum(ari|are|ara|e|ein)?\b/gi, (m) => "ham" + m.slice(3)));
  } else if (/\bham(ari|are|ara|e|ein)?\b/i.test(track)) {
    tracks.add(track.replace(/\bham(ari|are|ara|e|ein)?\b/gi, (m) => "hum" + m.slice(3)));
  }

  // Variant 2: "adhoori" <-> "adhuri"
  for (const t of Array.from(tracks)) {
    if (/adhoori/i.test(t)) {
      tracks.add(t.replace(/adhoori/gi, "adhuri"));
    } else if (/adhuri/i.test(t)) {
      tracks.add(t.replace(/adhuri/gi, "adhoori"));
    }
  }

  // Variant 3: Strip any "[from ...]" or "(from ...)"
  for (const t of Array.from(tracks)) {
    const stripped = t.replace(/\s*[\(\[]\s*from\s+[^\]\)]+[\)\]]/gi, "").trim();
    if (stripped && stripped !== t) {
      tracks.add(stripped);
    }
  }

  for (const t of tracks) {
    list.push({
      track: t,
      artist,
      q: [t, artist].filter(Boolean).join(" "),
    });
  }

  // Fallback: pure track queries without artist restriction
  for (const t of tracks) {
    if (artist) {
      list.push({
        track: t,
        artist: "",
        q: t,
      });
    }
  }

  return list;
}

/**
 * Find lyrics for an item.
 * 1. Cleans title of YouTube/promotional junk.
 * 2. Generates spelling / transliteration candidates.
 * 3. Tries exact /api/get, search /api/search with track & artist, and full-text /api/search?q=...
 */
export async function findLyrics(item: LyricsSource): Promise<LyricsResult | null> {
  const rawTitle = (item.title || "").trim();
  if (!rawTitle) return null;

  const cleanedTitle = cleanTrackTitle(rawTitle);
  const { trackName, artistName } = extractArtistAndTrack(cleanedTitle, item.artist);

  const rawDuration = Math.round(item.durationMs / 1000);
  const duration = rawDuration >= 1 && rawDuration <= 3600 ? rawDuration : undefined;

  const candidates = generateQueryCandidates(trackName, artistName);

  // Try each candidate variant
  for (const candidate of candidates) {
    const currTrack = candidate.track;
    const currArtist = candidate.artist;

    // Step 1: If artist is present, try exact /api/get
    if (currArtist && !/unknown/i.test(currArtist)) {
      try {
        const exact = asRecord(await getJson("/api/get", {
          track_name: currTrack,
          artist_name: currArtist,
          album_name: item.album?.trim() || undefined,
          duration,
        }));
        const result = exact && toResult(exact);
        if (result) {
          return result;
        }
      } catch {
        // continue
      }

      // Step 2: Try /api/search?track_name=...&artist_name=...
      try {
        const matches = asRecordArray(await getJson("/api/search", {
          track_name: currTrack,
          artist_name: currArtist,
        }));
        const best = closestByDuration(matches, duration);
        if (best) {
          const res = toResult(best);
          if (res) {
            return res;
          }
          const detailed = asRecord(await getJson(`/api/get/${best.id}`, {}));
          const detailedRes = detailed && toResult(detailed);
          if (detailedRes) return detailedRes;
        }
      } catch {
        // continue
      }
    }

    // Step 3: Try full-text /api/search?q=...
    try {
      const qMatches = asRecordArray(await getJson("/api/search", {
        q: candidate.q,
      }));
      const best = closestByDuration(qMatches, duration);
      if (best) {
        const res = toResult(best);
        if (res) {
          return res;
        }
        const detailed = asRecord(await getJson(`/api/get/${best.id}`, {}));
        const detailedRes = detailed && toResult(detailed);
        if (detailedRes) return detailedRes;
      }
    } catch {
      // continue
    }
  }
  return null;
}
