export interface ArtistBio {
  name: string;
  description: string | null;
  biography: string;
  photoUrl: string | null;
  thumbnailUrl: string | null;
  wikipediaUrl: string | null;
}

const cache = new Map<string, ArtistBio | null>();

/** Clean artist name for search (e.g. remove "feat.", commas, splitters). */
function cleanArtistQuery(raw: string): string {
  let name = raw.trim();
  // If multiple artists listed with commas, take the primary one
  if (name.includes(",")) {
    name = name.split(",")[0].trim();
  }
  // Remove "feat.", "ft.", "featuring", "vs.", "&"
  name = name.replace(/\s+(feat\.?|ft\.?|featuring|vs\.?)\s+.+$/i, "").trim();
  if (name.includes(" & ")) {
    name = name.split(" & ")[0].trim();
  }
  return name;
}

interface WikipediaSummaryResponse {
  title?: string;
  description?: string;
  extract?: string;
  thumbnail?: {
    source: string;
    width: number;
    height: number;
  };
  originalimage?: {
    source: string;
    width: number;
    height: number;
  };
  content_urls?: {
    desktop?: {
      page: string;
    };
  };
}

interface WikipediaSearchResponse {
  query?: {
    search?: Array<{
      title: string;
      snippet: string;
    }>;
  };
}

/** Fetch artist information from Wikipedia API. */
export async function fetchArtistBio(artistName: string): Promise<ArtistBio | null> {
  const cleanName = cleanArtistQuery(artistName);
  if (!cleanName || /unknown|various|stream/i.test(cleanName)) return null;

  const cacheKey = cleanName.toLowerCase();
  if (cache.has(cacheKey)) {
    return cache.get(cacheKey) ?? null;
  }

  try {
    // 1st attempt: Direct page summary
    const formattedTitle = encodeURIComponent(cleanName.replace(/\s+/g, "_"));
    const directUrl = `https://en.wikipedia.org/api/rest_v1/page/summary/${formattedTitle}`;
    
    let res = await fetch(directUrl, {
      headers: {
        Accept: "application/json",
      },
    });

    // If 404, fallback to Wikipedia search to get the exact canonical title
    if (!res.ok && res.status === 404) {
      const searchUrl = `https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch=${encodeURIComponent(
        cleanName + " singer",
      )}&utf8=&format=json&origin=*`;
      const searchRes = await fetch(searchUrl);
      if (searchRes.ok) {
        const searchData = (await searchRes.json()) as WikipediaSearchResponse;
        const topResult = searchData.query?.search?.[0];
        if (topResult?.title) {
          const secondUrl = `https://en.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(
            topResult.title.replace(/\s+/g, "_"),
          )}`;
          res = await fetch(secondUrl, { headers: { Accept: "application/json" } });
        }
      }
    }

    if (!res.ok) {
      cache.set(cacheKey, null);
      return null;
    }

    const data = (await res.json()) as WikipediaSummaryResponse;
    if (!data.extract || !data.title) {
      cache.set(cacheKey, null);
      return null;
    }

    const bio: ArtistBio = {
      name: data.title,
      description: data.description || null,
      biography: data.extract,
      photoUrl: data.originalimage?.source || data.thumbnail?.source || null,
      thumbnailUrl: data.thumbnail?.source || null,
      wikipediaUrl: data.content_urls?.desktop?.page || null,
    };

    cache.set(cacheKey, bio);
    return bio;
  } catch (err) {
    console.warn("[Artist] Error fetching bio for:", cleanName, err);
    return null;
  }
}
