import { ref } from "vue";
import type { ArtistBio } from "../services/artist";
import { fetchArtistBio } from "../services/artist";

const open = ref(false);
const loading = ref(false);
const currentArtist = ref<string>("");
const bio = ref<ArtistBio | null>(null);
const error = ref<string | null>(null);

async function showArtist(artistName: string) {
  const name = artistName ? artistName.trim() : "";
  if (!name || /unknown/i.test(name)) return;

  currentArtist.value = name;
  open.value = true;
  loading.value = true;
  error.value = null;
  bio.value = null;

  try {
    const result = await fetchArtistBio(name);
    bio.value = result;
    if (!result) {
      error.value = `No biography found for "${name}".`;
    }
  } catch (err) {
    error.value = "Failed to load artist information.";
  } finally {
    loading.value = false;
  }
}

function closeArtist() {
  open.value = false;
}

export function useArtistBio() {
  return {
    open,
    loading,
    currentArtist,
    bio,
    error,
    showArtist,
    closeArtist,
  };
}
