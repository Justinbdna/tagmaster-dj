import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  ArrowLeft,
  Check,
  ChevronDown,
  ImagePlus,
  Link2,
  ListChecks,
  Loader2,
  Music,
  Search,
  ShieldAlert,
  Sparkles,
  Trash2,
  Upload,
  Wand2,
  X,
} from "lucide-react";
import {
  exportManualTrack,
  getApiKeysSettings,
  isTauriEnvironment,
  openArtworkFileDialog,
  readTrackTags,
  searchArtworkCandidates,
  searchTrackCandidates,
  toLocalAssetUrl,
} from "../../services/tauriBridge";
import type {
  ArtworkCandidate,
  AudioTrackItem,
  ExportFormatMode,
  ExportMode,
  HardwareProfile,
  ManualExportResult,
  QuickSearchSource,
  TrackCandidate,
  TrackMetadata,
} from "../../types/track";
import { formatDuration, formatFileSize } from "../../utils/format";
import { LocalAudioPlayer } from "./LocalAudioPlayer";

interface ReviewQueueViewProps {
  reviewTracks: AudioTrackItem[];
  selectedTrackId: string | null;
  onSelectTrack: (id: string) => void;
  exportMode: ExportMode;
  formatMode: ExportFormatMode;
  hardwareProfile: HardwareProfile;
  onTrackValidated: (
    trackId: string,
    metadata: TrackMetadata,
    localArtworkPath?: string,
  ) => void;
  onTrackResolved: (
    trackId: string,
    result: ManualExportResult,
    localArtworkPath?: string,
  ) => void;
  onBackToLibrary: () => void;
}

interface ManualFormState {
  title: string;
  artist: string;
  albumArtist: string;
  album: string;
  year: string;
  genre: string;
  trackNumber: string;
  discNumber: string;
  artworkUrl: string;
  localArtworkPath: string;
}

const PROTECTED_KEYWORD_LABELS: ReadonlyArray<[RegExp, string]> = [
  [/\bsped\s+up\b/i, "Sped Up"],
  [/\bslowed\b/i, "Slowed"],
  [/\breverb\b/i, "Reverb"],
  [/\bnightcore\b/i, "Nightcore"],
  [/\binstrumental\b/i, "Instrumental"],
  [/\bacoustic\b/i, "Acoustic"],
  [/\bchopped\b/i, "Chopped"],
  [/\bmashup\b/i, "Mashup"],
  [/\bremix\b/i, "Remix"],
  [/\blive\b/i, "Live"],
];

const MUSICAL_QUALIFIERS = new Set([
  "pt",
  "part",
  "vol",
  "v",
  "ver",
  "version",
  "no",
  "n",
  "cd",
  "disc",
  "disk",
  "act",
  "chapter",
  "remix",
  "mix",
  "edit",
  "ep",
]);

function detectProtectedKeywordLabel(input: string): string | null {
  for (const [pattern, label] of PROTECTED_KEYWORD_LABELS) {
    if (pattern.test(input)) {
      return label;
    }
  }
  return null;
}

function stripExtensionAndDuplicateSuffix(raw: string): string {
  let current = raw
    .trim()
    .replace(/\.(?:mp3|m4a|aac|flac|wav)$/i, "")
    .trimEnd();

  for (let i = 0; i < 4; i += 1) {
    const prev = current;

    const copyMatch = current.match(
      /\s*(?:[-–—]\s*)?(?:copy|copie)(?:\s*\d{1,2})?\s*$/i,
    );
    if (copyMatch && copyMatch.index !== undefined) {
      const candidate = current.slice(0, copyMatch.index).trimEnd();
      if (candidate) current = candidate;
    } else {
      const parenMatch = current.match(/\s*\(\d{1,2}\)\s*$/);
      if (parenMatch && parenMatch.index !== undefined) {
        const candidate = current.slice(0, parenMatch.index).trimEnd();
        if (candidate) current = candidate;
      } else {
        const dblMatch = current.match(/__+\d{1,2}\s*$/);
        if (dblMatch && dblMatch.index !== undefined) {
          const candidate = current.slice(0, dblMatch.index).trimEnd();
          if (candidate) current = candidate;
        } else {
          const singleMatch = current.match(/_\d{1,2}\s*$/);
          if (singleMatch && singleMatch.index !== undefined) {
            const prefix = current.slice(0, singleMatch.index).trimEnd();
            const lastWord = (prefix.split(/[\s_\-/([]+/).pop() ?? "")
              .replace(/\.+$/, "")
              .toLowerCase();
            if (prefix && !MUSICAL_QUALIFIERS.has(lastWord)) {
              current = prefix;
            }
          }
        }
      }
    }

    if (current === prev) break;
  }

  return current
    .replace(
      /[\(\[](?:exclus?|unreleased|tiktok|prod\.?(?:\s+by)?\s+[^\)\]]+)[\)\]]/gi,
      " ",
    )
    .replace(/_/g, " ")
    .replace(/^\s*[!¡]*\s*(?:unreleased|exclus?)\s*(?:[-–—:]\s*|\s+)/i, "")
    .replace(/^\s*(?:\d{1,2}\s*[-.]\s*)?\d{1,3}\s*[-._)\]]*\s*/, "")
    .replace(/\s+/g, " ")
    .trim();
}

function buildEnrichedQuickQuery(
  fileName: string,
  artist?: string,
  albumArtist?: string,
  title?: string,
): string {
  const cleanArtist = (artist?.trim() || albumArtist?.trim() || "").replace(
    /\s+/g,
    " ",
  );
  const cleanTitle = title?.trim()
    ? stripExtensionAndDuplicateSuffix(title)
    : stripExtensionAndDuplicateSuffix(fileName);

  if (cleanArtist && cleanTitle) {
    if (cleanTitle.toLowerCase().includes(cleanArtist.toLowerCase())) {
      return cleanTitle;
    }
    return `${cleanArtist} ${cleanTitle}`.replace(/\s+/g, " ").trim();
  }

  return cleanTitle || stripExtensionAndDuplicateSuffix(fileName);
}

function emptyFormForTrack(track: AudioTrackItem): ManualFormState {
  const cleaned = stripExtensionAndDuplicateSuffix(track.fileName);
  const sepIdx = cleaned.indexOf(" - ");
  const artist = sepIdx > 0 ? cleaned.slice(0, sepIdx).trim() : "";
  const title = sepIdx > 0 ? cleaned.slice(sepIdx + 3).trim() : cleaned;

  return {
    title,
    artist,
    albumArtist: artist,
    album: "",
    year: "",
    genre: "",
    trackNumber: "",
    discNumber: "",
    artworkUrl: "",
    localArtworkPath: "",
  };
}

function parseOptionalInt(value: string): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  const parsed = Number.parseInt(trimmed, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

function getTrackStatusInfo(track: AudioTrackItem): {
  label: string;
  badgeClass: string;
} {
  if (track.status === "completed" && track.exportedPath) {
    return { label: "Exporté", badgeClass: "badge-success" };
  }
  if (track.status === "completed") {
    return { label: "Identifié", badgeClass: "badge-success" };
  }
  if (track.status === "failed") {
    return { label: "Erreur", badgeClass: "badge-danger" };
  }
  if (track.status === "needs_review") {
    return { label: "À réviser", badgeClass: "badge-warning" };
  }
  return {
    label: "En attente",
    badgeClass:
      "border-[var(--content-border)] bg-[var(--content-card-bg)] text-secondary",
  };
}

function candidateToMetadata(candidate: TrackCandidate): TrackMetadata {
  return {
    title: candidate.title,
    artist: candidate.artist,
    albumArtist: candidate.albumArtist || candidate.artist,
    album: candidate.album || candidate.title,
    trackNumber: candidate.trackNumber,
    trackTotal: candidate.trackTotal,
    discNumber: candidate.discNumber,
    discTotal: candidate.discTotal,
    year: candidate.year,
    genre: candidate.genre,
    artworkUrl: candidate.artworkUrl,
    isCompilation: candidate.isCompilation,
    source: candidate.source ?? "apple_music",
  };
}

const QUICK_SEARCH_SOURCES: { id: QuickSearchSource; label: string }[] = [
  { id: "auto", label: "Auto" },
  { id: "apple_music", label: "Apple Music" },
  { id: "deezer", label: "Deezer" },
  { id: "youtube", label: "YouTube" },
  { id: "soundcloud", label: "SoundCloud" },
  { id: "music_brainz", label: "MusicBrainz" },
  { id: "discogs", label: "Discogs" },
];

export function ReviewQueueView({
  reviewTracks,
  selectedTrackId,
  onSelectTrack,
  exportMode,
  formatMode,
  hardwareProfile,
  onTrackValidated,
  onTrackResolved,
  onBackToLibrary,
}: ReviewQueueViewProps) {
  const activeTrack = useMemo(() => {
    if (reviewTracks.length === 0) return null;
    return (
      reviewTracks.find((t) => t.id === selectedTrackId) ?? reviewTracks[0]
    );
  }, [reviewTracks, selectedTrackId]);

  const [quickQuery, setQuickQuery] = useState("");
  const [searchSource, setSearchSource] = useState<QuickSearchSource>("auto");
  const [sourceMenuOpen, setSourceMenuOpen] = useState(false);
  const [hasDiscogsToken, setHasDiscogsToken] = useState(false);
  const [suggestions, setSuggestions] = useState<TrackCandidate[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [exportingKey, setExportingKey] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [protectedLabel, setProtectedLabel] = useState<string | null>(null);
  const [isFormHighlighted, setIsFormHighlighted] = useState(false);
  const [showArtworkPicker, setShowArtworkPicker] = useState(false);
  const [artworkQuery, setArtworkQuery] = useState("");
  const [artworkResults, setArtworkResults] = useState<ArtworkCandidate[]>([]);
  const [isSearchingArtwork, setIsSearchingArtwork] = useState(false);
  const [artworkPickerError, setArtworkPickerError] = useState<string | null>(
    null,
  );
  const [directArtworkUrl, setDirectArtworkUrl] = useState("");
  const [isApplyingArtworkUrl, setIsApplyingArtworkUrl] = useState(false);
  const [directUrlError, setDirectUrlError] = useState<string | null>(null);
  const [previewLoadError, setPreviewLoadError] = useState(false);
  const [form, setForm] = useState<ManualFormState>({
    title: "",
    artist: "",
    albumArtist: "",
    album: "",
    year: "",
    genre: "",
    trackNumber: "",
    discNumber: "",
    artworkUrl: "",
    localArtworkPath: "",
  });

  const quickQueryDirtyRef = useRef(false);
  const sourceMenuRef = useRef<HTMLDivElement | null>(null);
  const rightPaneRef = useRef<HTMLElement | null>(null);
  const manualFormRef = useRef<HTMLFormElement | null>(null);
  const titleInputRef = useRef<HTMLInputElement | null>(null);
  const highlightTimerRef = useRef<number | null>(null);
  const focusTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (!isTauriEnvironment()) return;
    let active = true;
    getApiKeysSettings()
      .then((settings) => {
        if (active) setHasDiscogsToken(settings.hasDiscogsToken);
      })
      .catch(() => {
        // Ignore
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!sourceMenuOpen) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (
        sourceMenuRef.current &&
        !sourceMenuRef.current.contains(event.target as Node)
      ) {
        setSourceMenuOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSourceMenuOpen(false);
    };
    window.addEventListener("mousedown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [sourceMenuOpen]);

  useEffect(() => {
    return () => {
      if (highlightTimerRef.current !== null) {
        window.clearTimeout(highlightTimerRef.current);
      }
      if (focusTimerRef.current !== null) {
        window.clearTimeout(focusTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!activeTrack) return;

    quickQueryDirtyRef.current = false;
    setIsFormHighlighted(false);
    setShowArtworkPicker(false);
    setArtworkQuery("");
    setArtworkResults([]);
    setArtworkPickerError(null);
    setDirectArtworkUrl("");
    setIsApplyingArtworkUrl(false);
    setDirectUrlError(null);
    setPreviewLoadError(false);
    setSuggestions(activeTrack.candidates ?? []);
    setActionError(null);
    setSearchSource("auto");
    setSourceMenuOpen(false);
    setProtectedLabel(detectProtectedKeywordLabel(activeTrack.fileName));
    rightPaneRef.current?.scrollTo({ top: 0, behavior: "instant" });

    // Si la piste possède déjà des métadonnées résolues (Identifié / Exporté),
    // pré-remplir directement depuis celles-ci sans écraser avec les anciens tags disque.
    if (activeTrack.metadata) {
      const existingMeta = activeTrack.metadata;
      const metaKeyword = detectProtectedKeywordLabel(existingMeta.title);
      if (metaKeyword) {
        setProtectedLabel((prev) => prev ?? metaKeyword);
      }

      const cleanTitle = existingMeta.title
        .replace(
          /(?:\s*[\(\[]\s*|\s+)(?:feat\.?|ft\.?|featuring)\s+[^\)\]]+[\)\]]?/gi,
          "",
        )
        .trim();
      const metaQuery = [existingMeta.artist, cleanTitle || existingMeta.title]
        .filter(Boolean)
        .join(" ")
        .replace(/\s+/g, " ")
        .trim();

      setQuickQuery(metaQuery);
      setForm({
        title: existingMeta.title,
        artist: existingMeta.artist,
        albumArtist: existingMeta.albumArtist || existingMeta.artist,
        album: existingMeta.album,
        year: existingMeta.year ? String(existingMeta.year) : "",
        genre: existingMeta.genre ?? "",
        trackNumber: existingMeta.trackNumber
          ? String(existingMeta.trackNumber)
          : "",
        discNumber: existingMeta.discNumber
          ? String(existingMeta.discNumber)
          : "",
        artworkUrl: existingMeta.artworkUrl ?? "",
        localArtworkPath: activeTrack.localArtworkPath ?? "",
      });
      return;
    }

    const initialClean = activeTrack.lastQueryUsed?.trim()
      ? stripExtensionAndDuplicateSuffix(activeTrack.lastQueryUsed)
      : stripExtensionAndDuplicateSuffix(activeTrack.fileName);

    setQuickQuery(initialClean);
    setForm(emptyFormForTrack(activeTrack));

    if (!isTauriEnvironment()) return;
    let cancelled = false;
    const targetPath = activeTrack.sourcePath;
    const fileName = activeTrack.fileName;

    readTrackTags(targetPath)
      .then((existing) => {
        if (cancelled) return;

        const tagKeyword = existing.title
          ? detectProtectedKeywordLabel(existing.title)
          : null;
        if (tagKeyword) {
          setProtectedLabel((prev) => prev ?? tagKeyword);
        }

        if (!quickQueryDirtyRef.current) {
          const enriched = buildEnrichedQuickQuery(
            fileName,
            existing.artist,
            existing.albumArtist,
            existing.title,
          );
          if (enriched) {
            setQuickQuery(enriched);
          }
        }

        const rawAlbumArtist = existing.albumArtist?.trim() || "";
        const rawTitle = existing.title?.trim() || "";
        const validExistingAlbumArtist =
          rawAlbumArtist &&
          rawAlbumArtist.toLowerCase() !== rawTitle.toLowerCase()
            ? rawAlbumArtist
            : "";

        setForm((prev) => {
          const nextLocal =
            !prev.localArtworkPath &&
            !prev.artworkUrl &&
            existing.embeddedArtworkPath
              ? existing.embeddedArtworkPath
              : prev.localArtworkPath;
          return {
            ...prev,
            title: rawTitle || prev.title,
            artist: existing.artist?.trim() || prev.artist,
            albumArtist:
              validExistingAlbumArtist ||
              existing.artist?.trim() ||
              prev.albumArtist,
            album: existing.album?.trim() || prev.album,
            localArtworkPath: nextLocal,
          };
        });
      })
      .catch(() => {
        // Ignore read error when file has no readable tags
      });

    return () => {
      cancelled = true;
    };
  }, [activeTrack?.id]);

  if (reviewTracks.length === 0 || !activeTrack) {
    return (
      <section className="content-surface flex flex-1 flex-col items-center justify-center rounded-[24px] p-8 text-center">
        <div className="liquid-glass mb-5 flex h-16 w-16 items-center justify-center rounded-[20px]">
          <ListChecks
            className="relative z-10 h-7 w-7 text-sky-400"
            strokeWidth={1.85}
          />
        </div>
        <h3 className="text-[19px] font-semibold tracking-tight text-primary">
          Aucune piste en attente de révision
        </h3>
        <p className="mt-2 max-w-md text-[13px] leading-relaxed text-secondary">
          Toutes vos pistes douteuses ont été traitées ou aucun conflit n&apos;a
          été détecté pendant le lot.
        </p>
        <button
          type="button"
          onClick={onBackToLibrary}
          className="liquid-glass-pill mt-6 flex cursor-pointer items-center gap-2 rounded-full px-4 py-2 text-[12.5px] font-medium text-primary"
        >
          <ArrowLeft className="h-4 w-4 text-sky-400" />
          <span>Retour à la Bibliothèque</span>
        </button>
      </section>
    );
  }

  const handleQuickSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleaned = quickQuery.trim();
    if (!cleaned || isSearching) return;
    if (!isTauriEnvironment()) {
      setActionError(
        "La recherche rapide nécessite l'application desktop Tauri.",
      );
      return;
    }

    setActionError(null);
    setIsSearching(true);
    try {
      const results = await searchTrackCandidates(searchSource, cleaned);
      setSuggestions(results);
      if (results.length === 0) {
        setActionError(
          `Aucun résultat trouvé pour « ${cleaned} ». Essayez une autre source ou raccourcissez la requête.`,
        );
      }
    } catch (err) {
      setActionError(
        err instanceof Error
          ? err.message
          : "Erreur pendant la recherche rapide.",
      );
    } finally {
      setIsSearching(false);
    }
  };

  const handleValidateCandidate = (candidate: TrackCandidate) => {
    setActionError(null);
    onTrackValidated(activeTrack.id, candidateToMetadata(candidate));
  };

  const handleApplyCandidate = async (
    candidate: TrackCandidate,
    idx: number,
  ) => {
    if (exportingKey) return;
    if (!isTauriEnvironment()) {
      setActionError(
        "L'export non destructif nécessite l'application desktop Tauri.",
      );
      return;
    }

    setActionError(null);
    setExportingKey(`candidate-${idx}`);
    try {
      const result = await exportManualTrack({
        sourcePath: activeTrack.sourcePath,
        exportMode,
        formatMode,
        hardwareProfile,
        title: candidate.title,
        artist: candidate.artist,
        albumArtist: candidate.albumArtist,
        album: candidate.album,
        year: candidate.year,
        genre: candidate.genre,
        trackNumber: candidate.trackNumber,
        trackTotal: candidate.trackTotal,
        discNumber: candidate.discNumber,
        discTotal: candidate.discTotal,
        isCompilation: candidate.isCompilation,
        artworkUrl: candidate.artworkUrl,
        source: candidate.source ?? "apple_music",
      });
      onTrackResolved(activeTrack.id, result);
    } catch (err) {
      setActionError(
        err instanceof Error
          ? err.message
          : "Impossible d'appliquer et d'exporter cette suggestion.",
      );
    } finally {
      setExportingKey(null);
    }
  };

  const handlePrefillFromCandidate = (candidate: TrackCandidate) => {
    setActionError(null);

    let nextTitle = candidate.title;
    if (
      protectedLabel &&
      detectProtectedKeywordLabel(candidate.title)?.toLowerCase() !==
        protectedLabel.toLowerCase()
    ) {
      nextTitle = `${candidate.title} (${protectedLabel})`;
    }

    setPreviewLoadError(false);
    setForm({
      title: nextTitle,
      artist: candidate.artist,
      albumArtist: candidate.albumArtist || candidate.artist,
      album: candidate.album,
      year: candidate.year ? String(candidate.year) : "",
      genre: candidate.genre ?? "",
      trackNumber: candidate.trackNumber ? String(candidate.trackNumber) : "",
      discNumber: candidate.discNumber ? String(candidate.discNumber) : "",
      artworkUrl: candidate.artworkUrl ?? "",
      localArtworkPath: "",
    });

    if (highlightTimerRef.current !== null) {
      window.clearTimeout(highlightTimerRef.current);
    }
    if (focusTimerRef.current !== null) {
      window.clearTimeout(focusTimerRef.current);
    }

    setIsFormHighlighted(true);
    highlightTimerRef.current = window.setTimeout(() => {
      setIsFormHighlighted(false);
    }, 1200);

    manualFormRef.current?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });

    focusTimerRef.current = window.setTimeout(() => {
      const input = titleInputRef.current;
      if (input) {
        input.focus({ preventScroll: true });
        const len = input.value.length;
        input.setSelectionRange(len, len);
      }
    }, 100);
  };

  const handlePickLocalArtwork = async () => {
    if (!isTauriEnvironment()) return;
    try {
      const selected = await openArtworkFileDialog();
      if (selected) {
        setPreviewLoadError(false);
        setForm((prev) => ({ ...prev, localArtworkPath: selected }));
      }
    } catch (err) {
      setActionError(
        err instanceof Error
          ? err.message
          : "Impossible d'ouvrir le sélecteur d'image.",
      );
    }
  };

  const handleApplyDirectArtworkUrl = async () => {
    const trimmed = directArtworkUrl.trim();
    if (!trimmed || isApplyingArtworkUrl) return;

    let parsed: URL;
    try {
      parsed = new URL(trimmed);
    } catch {
      setDirectUrlError(
        "URL invalide. Utilisez un lien commençant par https:// ou http://.",
      );
      return;
    }

    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      setDirectUrlError("Seules les URLs http:// ou https:// sont acceptées.");
      return;
    }

    setDirectUrlError(null);
    setIsApplyingArtworkUrl(true);
    try {
      await new Promise<void>((resolve, reject) => {
        const img = new Image();
        const timeoutId = window.setTimeout(() => {
          img.src = "";
          reject(new Error("Délai dépassé lors du chargement de l'image."));
        }, 8000);
        img.onload = () => {
          window.clearTimeout(timeoutId);
          resolve();
        };
        img.onerror = () => {
          window.clearTimeout(timeoutId);
          reject(
            new Error(
              "Impossible de charger cette image — vérifiez qu'il s'agit d'un lien direct (JPG, PNG, WEBP).",
            ),
          );
        };
        img.src = parsed.toString();
      });

      setPreviewLoadError(false);
      setForm((prev) => ({
        ...prev,
        artworkUrl: parsed.toString(),
        localArtworkPath: "",
      }));
      setShowArtworkPicker(false);
    } catch (err) {
      setDirectUrlError(
        err instanceof Error
          ? err.message
          : "Impossible de charger cette URL d'image.",
      );
    } finally {
      setIsApplyingArtworkUrl(false);
    }
  };

  const handleSearchArtworks = async (overrideQuery?: string) => {
    const cleaned = (overrideQuery ?? artworkQuery).trim();
    if (!cleaned || isSearchingArtwork) return;
    if (!isTauriEnvironment()) {
      setArtworkPickerError(
        "La recherche de pochettes nécessite l'application desktop Tauri.",
      );
      return;
    }

    setArtworkPickerError(null);
    setIsSearchingArtwork(true);
    try {
      const covers = await searchArtworkCandidates(cleaned);
      setArtworkResults(covers);
      if (covers.length === 0) {
        setArtworkPickerError(
          `Aucune pochette carrée trouvée pour « ${cleaned} ».`,
        );
      }
    } catch (err) {
      setArtworkPickerError(
        err instanceof Error
          ? err.message
          : "Erreur pendant la recherche de pochettes.",
      );
    } finally {
      setIsSearchingArtwork(false);
    }
  };

  const handleToggleArtworkPicker = () => {
    const nextOpen = !showArtworkPicker;
    setShowArtworkPicker(nextOpen);
    if (nextOpen) {
      const defaultQuery =
        [form.artist.trim(), form.title.trim()].filter(Boolean).join(" ") ||
        quickQuery.trim();
      setArtworkQuery(defaultQuery);
      setDirectArtworkUrl(form.artworkUrl || "");
      setDirectUrlError(null);
      setArtworkPickerError(null);
      if (
        artworkResults.length === 0 &&
        defaultQuery &&
        isTauriEnvironment()
      ) {
        void handleSearchArtworks(defaultQuery);
      }
    }
  };

  const handleManualValidate = () => {
    const title = form.title.trim();
    const artist = form.artist.trim();
    if (!title || !artist) {
      setActionError("Le titre et l'artiste sont obligatoires.");
      return;
    }

    setActionError(null);
    const metadata: TrackMetadata = {
      title,
      artist,
      albumArtist: form.albumArtist.trim() || artist,
      album: form.album.trim() || title,
      year: parseOptionalInt(form.year),
      genre: form.genre.trim() || undefined,
      trackNumber: parseOptionalInt(form.trackNumber),
      discNumber: parseOptionalInt(form.discNumber),
      artworkUrl: form.localArtworkPath
        ? undefined
        : form.artworkUrl.trim() || undefined,
      isCompilation: false,
      source: "manual",
    };

    onTrackValidated(
      activeTrack.id,
      metadata,
      form.localArtworkPath || undefined,
    );
  };

  const handleManualExport = async (e: React.FormEvent) => {
    e.preventDefault();
    if (exportingKey) return;
    if (!isTauriEnvironment()) {
      setActionError(
        "L'export non destructif nécessite l'application desktop Tauri.",
      );
      return;
    }

    const title = form.title.trim();
    const artist = form.artist.trim();
    if (!title || !artist) {
      setActionError("Le titre et l'artiste sont obligatoires.");
      return;
    }

    setActionError(null);
    setExportingKey("manual");
    try {
      const result = await exportManualTrack({
        sourcePath: activeTrack.sourcePath,
        exportMode,
        formatMode,
        hardwareProfile,
        title,
        artist,
        albumArtist: form.albumArtist.trim() || artist,
        album: form.album.trim() || undefined,
        year: parseOptionalInt(form.year),
        genre: form.genre.trim() || undefined,
        trackNumber: parseOptionalInt(form.trackNumber),
        discNumber: parseOptionalInt(form.discNumber),
        artworkUrl: form.localArtworkPath
          ? undefined
          : form.artworkUrl.trim() || undefined,
        localArtworkPath: form.localArtworkPath || undefined,
        source: "manual",
      });
      onTrackResolved(
        activeTrack.id,
        result,
        form.localArtworkPath || undefined,
      );
    } catch (err) {
      setActionError(
        err instanceof Error
          ? err.message
          : "Impossible d'enregistrer et d'exporter la piste.",
      );
    } finally {
      setExportingKey(null);
    }
  };

  const previewArtworkSrc = form.localArtworkPath
    ? toLocalAssetUrl(form.localArtworkPath)
    : form.artworkUrl || null;
  const activeStatus = getTrackStatusInfo(activeTrack);
  const onlyReviewTracks = reviewTracks.every(
    (t) => t.status === "needs_review" || t.status === "failed",
  );

  return (
    <div className="flex min-h-0 flex-1 gap-3.5">
      {/* Left Column: Compact queue of tracks to review/edit */}
      <aside className="content-surface flex w-56 shrink-0 flex-col overflow-hidden rounded-[24px]">
        <div className="border-b border-[var(--content-divider)] px-4 py-3">
          <h3 className="text-[13px] font-semibold text-primary">
            {reviewTracks.length} piste{reviewTracks.length > 1 ? "s" : ""}{" "}
            {onlyReviewTracks ? "à réviser" : "à éditer"}
          </h3>
        </div>

        <ul className="custom-scrollbar flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto p-2">
          {reviewTracks.map((track) => {
            const isSelected = track.id === activeTrack.id;
            const statusInfo = getTrackStatusInfo(track);
            return (
              <li key={track.id}>
                <button
                  type="button"
                  onClick={() => onSelectTrack(track.id)}
                  className={`flex w-full cursor-pointer items-start gap-2 rounded-[13px] p-2 text-left transition-all ${
                    isSelected
                      ? "liquid-glass-active"
                      : "border border-transparent hover:bg-[var(--content-card-hover)]"
                  }`}
                >
                  <div
                    className={`mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-[9px] border ${statusInfo.badgeClass}`}
                  >
                    {track.status === "failed" ? (
                      <AlertTriangle className="h-3.5 w-3.5" />
                    ) : track.status === "needs_review" ? (
                      <ShieldAlert className="h-3.5 w-3.5" />
                    ) : track.status === "completed" ? (
                      <Check className="h-3.5 w-3.5" />
                    ) : (
                      <Music className="h-3.5 w-3.5" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p
                      className="truncate text-[12px] font-medium text-primary"
                      title={track.fileName}
                    >
                      {track.metadata
                        ? `${track.metadata.artist} — ${track.metadata.title}`
                        : track.fileName}
                    </p>
                    <p className="mt-0.5 truncate text-[10.5px] text-muted">
                      {statusInfo.label} · .{track.extension}
                    </p>
                  </div>
                </button>
              </li>
            );
          })}
        </ul>
      </aside>

      {/* Right Column: Interactive 3-level resolution workspace */}
      <section
        ref={rightPaneRef}
        className="content-surface custom-scrollbar flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto rounded-[24px] p-5"
      >
        {/* Header + Local Audio Player */}
        <div className="flex flex-col gap-3 rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h3
                className="truncate text-[15px] font-semibold text-primary"
                title={activeTrack.sourcePath}
              >
                {activeTrack.fileName}
              </h3>
              <span className="rounded-md border border-[var(--content-border)] bg-[var(--content-card-bg)] px-2 py-0.5 text-[10px] font-semibold tracking-wider text-secondary">
                .{activeTrack.extension}
              </span>
              <span
                className={`rounded-full border px-2 py-0.5 text-[10px] font-semibold ${activeStatus.badgeClass}`}
              >
                {activeStatus.label}
              </span>
            </div>
            <p className="mt-1 truncate text-[11.5px] text-muted">
              Dossier : {activeTrack.parentFolder} ·{" "}
              {formatFileSize(activeTrack.sizeBytes)}
            </p>
            {activeTrack.reviewReason && (
              <p className="text-semantic-warning mt-1.5 text-[12px] font-medium">
                {activeTrack.reviewReason}
              </p>
            )}
          </div>

          <LocalAudioPlayer
            sourcePath={activeTrack.sourcePath}
            fileName={activeTrack.fileName}
          />
        </div>

        {actionError && (
          <div className="rounded-[14px] border badge-danger px-4 py-2.5 text-[12px]">
            {actionError}
          </div>
        )}

        {/* Mécanisme 2 : Barre de recherche rapide à la volée */}
        <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
          <div className="mb-2.5 flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2 text-[13px] font-semibold text-primary">
              <Search className="h-4 w-4 text-sky-400" />
              <h4>Recherche rapide</h4>
            </div>

            {/* Unified 7-source custom Liquid Glass popover */}
            <div ref={sourceMenuRef} className="relative flex items-center gap-2">
              <span className="text-[11px] font-medium text-muted">
                Source
              </span>
              <button
                type="button"
                disabled={isSearching}
                onClick={() => setSourceMenuOpen((prev) => !prev)}
                aria-haspopup="listbox"
                aria-expanded={sourceMenuOpen}
                aria-label="Source de recherche rapide"
                className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[11.5px] font-semibold text-primary disabled:cursor-not-allowed disabled:opacity-50"
              >
                <span>
                  {QUICK_SEARCH_SOURCES.find((s) => s.id === searchSource)
                    ?.label ?? "Auto"}
                </span>
                <ChevronDown
                  className={`h-3.5 w-3.5 text-secondary transition-transform duration-150 ${
                    sourceMenuOpen ? "rotate-180" : ""
                  }`}
                />
              </button>

              {sourceMenuOpen && (
                <div
                  role="listbox"
                  aria-label="Choisir une source de recherche"
                  className="modal-surface absolute right-0 top-full z-40 mt-1.5 w-52 rounded-[16px] p-1.5"
                >
                  {QUICK_SEARCH_SOURCES.map((opt) => {
                    const isSelected = searchSource === opt.id;
                    const needsToken = opt.id === "discogs" && !hasDiscogsToken;
                    return (
                      <button
                        key={opt.id}
                        type="button"
                        role="option"
                        aria-selected={isSelected}
                        onClick={() => {
                          setSearchSource(opt.id);
                          setSourceMenuOpen(false);
                        }}
                        className={`flex w-full cursor-pointer items-center justify-between gap-2 rounded-[10px] px-3 py-1.5 text-left text-[12px] font-medium transition-colors ${
                          isSelected
                            ? "bg-sky-500/15 text-primary"
                            : "text-secondary hover:bg-[var(--content-card-hover)] hover:text-primary"
                        }`}
                      >
                        <span className="flex min-w-0 items-center gap-1.5 truncate">
                          <span>{opt.label}</span>
                          {needsToken && (
                            <span className="text-[10px] font-normal text-muted">
                              (token requis)
                            </span>
                          )}
                        </span>
                        {isSelected && (
                          <Check className="text-semantic-info h-3.5 w-3.5 shrink-0" />
                        )}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          <form
            onSubmit={(e) => void handleQuickSearch(e)}
            className="flex gap-2"
          >
            <input
              type="text"
              value={quickQuery}
              onChange={(e) => {
                quickQueryDirtyRef.current = true;
                setQuickQuery(e.target.value);
              }}
              placeholder={
                searchSource === "youtube" || searchSource === "soundcloud"
                  ? "Ex: Daft Punk Get Lucky Remix ou Bad Bunny Live"
                  : "Ex: Bad Bunny NUEVAYoL ou Shay - T mimi"
              }
              className="min-w-0 flex-1 rounded-full border border-[var(--content-border)] bg-[var(--content-bg)] px-4 py-2 text-[12.5px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none"
            />
            <button
              type="submit"
              disabled={isSearching || !quickQuery.trim()}
              className="liquid-glass-pill flex shrink-0 cursor-pointer items-center gap-2 rounded-full px-4 py-2 text-[12.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isSearching ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin text-sky-400" />
                  <span>Recherche…</span>
                </>
              ) : (
                <>
                  <Search className="h-3.5 w-3.5 text-sky-400" />
                  <span>Rechercher</span>
                </>
              )}
            </button>
          </form>
        </div>

        {/* Mécanisme 1 : Suggestions probables (Top 3 à 5, mise en page sans troncature) */}
        <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
          <div className="mb-3 flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 text-[13px] font-semibold text-primary">
              <Sparkles className="h-4 w-4 text-sky-400" />
              <h4>Suggestions probables ({suggestions.length})</h4>
            </div>
            <span className="text-[11px] text-muted">
              Éditer, valider en lot ou exporter directement
            </span>
          </div>

          {suggestions.length === 0 ? (
            <p className="rounded-[12px] border border-dashed border-[var(--content-border)] px-4 py-4 text-center text-[12px] text-muted">
              Aucune suggestion automatique disponible pour l&apos;instant —
              utilisez la barre de recherche rapide ci-dessus ou le formulaire
              manuel ci-dessous.
            </p>
          ) : (
            <ul className="flex flex-col gap-2.5">
              {suggestions.map((cand, idx) => {
                const key = `candidate-${idx}`;
                const isBusy = exportingKey === key;
                const confidencePct = Math.round(cand.confidenceScore * 100);
                const platformLabel =
                  cand.platform ??
                  (cand.source === "deezer"
                    ? "Deezer"
                    : cand.source === "music_brainz"
                      ? "MusicBrainz"
                      : cand.source === "discogs"
                        ? "Discogs"
                        : cand.source === "yt_dlp"
                          ? cand.storefront
                          : `Apple Music ${cand.storefront}`);
                const sourceBadgeClass =
                  platformLabel === "YouTube"
                    ? "badge-youtube"
                    : platformLabel === "SoundCloud"
                      ? "badge-soundcloud"
                      : platformLabel === "Deezer"
                        ? "badge-deezer"
                        : platformLabel === "MusicBrainz"
                          ? "badge-musicbrainz"
                          : platformLabel === "Discogs"
                            ? "badge-discogs"
                            : "badge-info";

                return (
                  <li
                    key={`${platformLabel}-${cand.artist}-${cand.title}-${cand.album}-${idx}`}
                    className="rounded-[14px] border border-[var(--content-border)] bg-[var(--content-bg)] p-3.5"
                  >
                    <div className="flex items-start gap-3">
                      {cand.artworkUrl ? (
                        <img
                          src={cand.artworkUrl}
                          alt={cand.album}
                          className="h-12 w-12 shrink-0 rounded-[10px] border border-[var(--content-border)] object-cover"
                        />
                      ) : (
                        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-[10px] border border-[var(--content-border)] bg-[var(--content-card-bg)] text-sky-400">
                          <Music className="h-4 w-4" />
                        </div>
                      )}

                      <div className="min-w-0 flex-1">
                        <div className="flex items-start justify-between gap-2">
                          <p className="text-[13.5px] font-semibold leading-snug text-primary">
                            {cand.artist} — {cand.title}
                          </p>
                          <span
                            className={`shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-semibold ${sourceBadgeClass}`}
                          >
                            {platformLabel} · {confidencePct}%
                          </span>
                        </div>
                        <p className="mt-1 text-[11.5px] text-secondary">
                          {cand.album}
                          {cand.year ? ` (${cand.year})` : ""}
                          {cand.genre ? ` · ${cand.genre}` : ""}
                          {cand.durationSecs
                            ? ` · ${formatDuration(cand.durationSecs)}`
                            : ""}
                        </p>
                      </div>
                    </div>

                    <div className="mt-2.5 flex flex-wrap items-center justify-end gap-2 border-t border-[var(--content-divider)] pt-2.5">
                      <button
                        type="button"
                        onClick={() => handlePrefillFromCandidate(cand)}
                        disabled={exportingKey !== null}
                        className="liquid-glass-pill cursor-pointer rounded-full px-3 py-1 text-[11.5px] font-medium text-secondary hover:text-primary disabled:opacity-50"
                      >
                        Éditer
                      </button>

                      <button
                        type="button"
                        onClick={() => handleValidateCandidate(cand)}
                        disabled={exportingKey !== null}
                        title="Valider les tags (passe à Identifié sans exporter tout de suite)"
                        className="liquid-glass-pill flex cursor-pointer items-center gap-1 rounded-full px-3 py-1 text-[11.5px] font-medium text-primary disabled:opacity-50"
                      >
                        <Check className="h-3.5 w-3.5 text-emerald-400" />
                        <span>Valider</span>
                      </button>

                      <button
                        type="button"
                        onClick={() => void handleApplyCandidate(cand, idx)}
                        disabled={exportingKey !== null}
                        className="liquid-glass-primary flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-1 text-[11.5px] font-semibold disabled:cursor-not-allowed disabled:opacity-55"
                      >
                        {isBusy ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                          <Upload className="h-3.5 w-3.5" />
                        )}
                        <span>Exporter</span>
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {/* Mécanisme 3 : Formulaire 100 % manuel + import pochette locale */}
        <form
          ref={manualFormRef}
          onSubmit={(e) => void handleManualExport(e)}
          className={`rounded-[18px] border bg-[var(--content-card-bg)] p-4 transition-all duration-300 ${
            isFormHighlighted
              ? "border-sky-400/75 ring-2 ring-sky-400/30 shadow-[0_0_28px_var(--accent-glow)]"
              : "border-[var(--content-border)]"
          }`}
        >
          <div className="mb-3 flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 text-[13px] font-semibold text-primary">
              <Wand2 className="h-4 w-4 text-sky-400" />
              <h4>Formulaire manuel &amp; pochette personnalisée</h4>
            </div>
            <span className="text-[11px] text-muted">
              Table Rase + export{" "}
              {exportMode === "dynamic" ? "Dynamique" : "Plat"}
            </span>
          </div>

          <div className="flex flex-col gap-4 lg:flex-row">
            {/* Artwork picker column */}
            <div className="flex w-full shrink-0 flex-col items-center gap-2.5 lg:w-[156px]">
              <div className="flex h-36 w-36 items-center justify-center overflow-hidden rounded-[16px] border border-[var(--content-border)] bg-[var(--content-bg)]">
                {previewArtworkSrc && !previewLoadError ? (
                  <img
                    src={previewArtworkSrc}
                    alt="Aperçu pochette"
                    onError={() => setPreviewLoadError(true)}
                    className="h-full w-full object-cover"
                  />
                ) : (
                  <div className="flex flex-col items-center gap-1 text-center text-muted">
                    <Music className="h-6 w-6" />
                    <span className="text-[10.5px]">Aucune pochette</span>
                  </div>
                )}
              </div>

              <div className="flex w-full flex-col gap-1.5">
                <button
                  type="button"
                  onClick={() => void handlePickLocalArtwork()}
                  disabled={exportingKey !== null}
                  className="liquid-glass-pill flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-full px-3 py-1.5 text-[11.5px] font-medium text-primary"
                >
                  <ImagePlus className="h-3.5 w-3.5 text-sky-400" />
                  <span>Choisir une image…</span>
                </button>

                <button
                  type="button"
                  onClick={handleToggleArtworkPicker}
                  disabled={exportingKey !== null}
                  className={`flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-full px-3 py-1.5 text-[11.5px] font-medium transition-all ${
                    showArtworkPicker
                      ? "liquid-glass-active text-primary"
                      : "liquid-glass-pill text-secondary hover:text-primary"
                  }`}
                >
                  <Sparkles className="h-3.5 w-3.5 text-sky-400" />
                  <span>Pochette HD en ligne</span>
                </button>

                {(form.localArtworkPath || form.artworkUrl) && (
                  <button
                    type="button"
                    onClick={() =>
                      setForm((prev) => ({
                        ...prev,
                        localArtworkPath: "",
                        artworkUrl: "",
                      }))
                    }
                    className="flex w-full cursor-pointer items-center justify-center gap-1 rounded-full py-1 text-[11px] text-muted hover:text-rose-400"
                  >
                    <Trash2 className="h-3 w-3" />
                    <span>Retirer la pochette</span>
                  </button>
                )}
              </div>
            </div>

            {/* Metadata inputs grid */}
            <div className="grid min-w-0 flex-1 gap-3 sm:grid-cols-2">
              <label className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-secondary">
                  Titre *
                </span>
                <input
                  ref={titleInputRef}
                  type="text"
                  required
                  value={form.title}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, title: e.target.value }))
                  }
                  className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary focus:border-sky-400/60 focus:outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-secondary">
                  Artiste *
                </span>
                <input
                  type="text"
                  required
                  value={form.artist}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, artist: e.target.value }))
                  }
                  className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary focus:border-sky-400/60 focus:outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-secondary">
                  Artiste de l&apos;album
                </span>
                <input
                  type="text"
                  value={form.albumArtist}
                  onChange={(e) =>
                    setForm((prev) => ({
                      ...prev,
                      albumArtist: e.target.value,
                    }))
                  }
                  placeholder="Verrouillé sur l'artiste principal si vide"
                  className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-secondary">
                  Album / Single
                </span>
                <input
                  type="text"
                  value={form.album}
                  onChange={(e) =>
                    setForm((prev) => ({ ...prev, album: e.target.value }))
                  }
                  placeholder="Reprend le titre si single"
                  className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none"
                />
              </label>

              <div className="grid grid-cols-2 gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-[11px] font-medium text-secondary">
                    Année
                  </span>
                  <input
                    type="number"
                    min={1900}
                    max={2100}
                    value={form.year}
                    onChange={(e) =>
                      setForm((prev) => ({ ...prev, year: e.target.value }))
                    }
                    className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary focus:border-sky-400/60 focus:outline-none"
                  />
                </label>

                <label className="flex flex-col gap-1">
                  <span className="text-[11px] font-medium text-secondary">
                    Genre
                  </span>
                  <input
                    type="text"
                    value={form.genre}
                    onChange={(e) =>
                      setForm((prev) => ({ ...prev, genre: e.target.value }))
                    }
                    placeholder="Pop, Hip-Hop/Rap…"
                    className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none"
                  />
                </label>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-[11px] font-medium text-secondary">
                    N° de piste
                  </span>
                  <input
                    type="number"
                    min={1}
                    value={form.trackNumber}
                    onChange={(e) =>
                      setForm((prev) => ({
                        ...prev,
                        trackNumber: e.target.value,
                      }))
                    }
                    className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary focus:border-sky-400/60 focus:outline-none"
                  />
                </label>

                <label className="flex flex-col gap-1">
                  <span className="text-[11px] font-medium text-secondary">
                    N° de disque
                  </span>
                  <input
                    type="number"
                    min={1}
                    value={form.discNumber}
                    onChange={(e) =>
                      setForm((prev) => ({
                        ...prev,
                        discNumber: e.target.value,
                      }))
                    }
                    className="rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3 py-2 text-[12.5px] text-primary focus:border-sky-400/60 focus:outline-none"
                  />
                </label>
              </div>
            </div>
          </div>

          {showArtworkPicker && (
            <div className="mt-4 rounded-[14px] border border-[var(--content-border)] bg-[var(--content-bg)] p-3.5">
              <div className="mb-3 flex items-center justify-between gap-2">
                <div className="flex items-center gap-2 text-[12px] font-semibold text-primary">
                  <Sparkles className="h-3.5 w-3.5 text-sky-400" />
                  <span>
                    Sélecteur de pochettes HD (jusqu&apos;à 16 résultats) ou URL
                    directe
                  </span>
                </div>
                <button
                  type="button"
                  onClick={() => setShowArtworkPicker(false)}
                  aria-label="Fermer le sélecteur de pochettes"
                  className="flex h-6 w-6 cursor-pointer items-center justify-center rounded-full text-muted hover:bg-white/10 hover:text-primary"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>

              {/* Direct web image URL input */}
              <div className="mb-2.5 flex flex-col gap-1.5">
                <div className="flex gap-2">
                  <div className="relative flex min-w-0 flex-1 items-center">
                    <Link2 className="pointer-events-none absolute left-3 h-3.5 w-3.5 text-muted" />
                    <input
                      type="url"
                      value={directArtworkUrl}
                      onFocus={(e) => e.currentTarget.select()}
                      onChange={(e) => {
                        setDirectUrlError(null);
                        setDirectArtworkUrl(e.target.value);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.preventDefault();
                          void handleApplyDirectArtworkUrl();
                        }
                      }}
                      placeholder="Coller une URL d'image directe (https://...)"
                      className={`min-w-0 w-full rounded-full border border-[var(--content-border)] bg-[var(--content-card-bg)] py-1.5 pl-8 text-[12px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none ${
                        directArtworkUrl ? "pr-8" : "pr-3.5"
                      }`}
                    />
                    {directArtworkUrl && (
                      <button
                        type="button"
                        onClick={() => {
                          setDirectArtworkUrl("");
                          setDirectUrlError(null);
                        }}
                        aria-label="Effacer l'URL d'image"
                        title="Effacer l'URL"
                        className="absolute right-2 flex h-5 w-5 cursor-pointer items-center justify-center rounded-full text-muted transition-colors hover:bg-white/10 hover:text-primary"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    )}
                  </div>
                  <button
                    type="button"
                    onClick={() => void handleApplyDirectArtworkUrl()}
                    disabled={isApplyingArtworkUrl || !directArtworkUrl.trim()}
                    className="liquid-glass-pill flex shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[11.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {isApplyingArtworkUrl ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin text-sky-400" />
                    ) : (
                      <Check className="h-3.5 w-3.5 text-emerald-400" />
                    )}
                    <span>Appliquer</span>
                  </button>
                </div>
                {directUrlError && (
                  <p className="text-semantic-warning text-[11.5px] font-medium">
                    {directUrlError}
                  </p>
                )}
              </div>

              {/* Apple Music 16-cover search */}
              <div className="flex gap-2">
                <input
                  type="text"
                  value={artworkQuery}
                  onChange={(e) => setArtworkQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      void handleSearchArtworks();
                    }
                  }}
                  placeholder="Rechercher sur Apple Music (Ex: Kim Petras, SOPHIE…)"
                  className="min-w-0 flex-1 rounded-full border border-[var(--content-border)] bg-[var(--content-card-bg)] px-3.5 py-1.5 text-[12px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => void handleSearchArtworks()}
                  disabled={isSearchingArtwork || !artworkQuery.trim()}
                  className="liquid-glass-pill flex shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[11.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {isSearchingArtwork ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin text-sky-400" />
                  ) : (
                    <Search className="h-3.5 w-3.5 text-sky-400" />
                  )}
                  <span>Chercher</span>
                </button>
              </div>

              {artworkPickerError && (
                <p className="text-semantic-warning mt-2 text-[11.5px] font-medium">
                  {artworkPickerError}
                </p>
              )}

              {artworkResults.length > 0 && (
                <div className="custom-scrollbar mt-3 grid max-h-72 grid-cols-2 gap-2.5 overflow-y-auto pr-1 sm:grid-cols-4">
                  {artworkResults.map((cover, idx) => {
                    const isPicked =
                      !form.localArtworkPath &&
                      form.artworkUrl === cover.artworkUrl;
                    return (
                      <button
                        key={`${cover.artworkUrl}-${idx}`}
                        type="button"
                        onClick={() => {
                          setPreviewLoadError(false);
                          setForm((prev) => ({
                            ...prev,
                            artworkUrl: cover.artworkUrl,
                            localArtworkPath: "",
                          }));
                          setShowArtworkPicker(false);
                        }}
                        title={`${cover.artist} — ${cover.album || cover.title}${cover.year ? ` (${cover.year})` : ""}`}
                        className={`group flex cursor-pointer flex-col gap-1.5 rounded-[12px] border p-2 text-left transition-all ${
                          isPicked
                            ? "border-sky-400 bg-sky-500/10"
                            : "border-[var(--content-border)] bg-[var(--content-card-bg)] hover:border-sky-400/60 hover:bg-[var(--content-card-hover)]"
                        }`}
                      >
                        <div className="aspect-square w-full overflow-hidden rounded-[8px] border border-[var(--content-border)]">
                          <img
                            src={cover.previewUrl}
                            alt={cover.album || cover.title}
                            className="h-full w-full object-cover transition-transform duration-200 group-hover:scale-105"
                          />
                        </div>
                        <div className="min-w-0">
                          <p className="truncate text-[11px] font-semibold text-primary">
                            {cover.album || cover.title}
                          </p>
                          <p className="truncate text-[10px] text-muted">
                            {cover.artist}
                            {cover.year ? ` · ${cover.year}` : ""}
                          </p>
                        </div>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          <div className="mt-4 flex flex-wrap items-center justify-end gap-2.5">
            <button
              type="button"
              onClick={handleManualValidate}
              disabled={
                exportingKey !== null ||
                !form.title.trim() ||
                !form.artist.trim()
              }
              title="Valider ces tags sans exporter tout de suite"
              className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Check className="h-4 w-4 text-emerald-400" />
              <span>Valider les tags</span>
            </button>

            <button
              type="submit"
              disabled={
                exportingKey !== null ||
                !form.title.trim() ||
                !form.artist.trim()
              }
              className="liquid-glass-primary flex cursor-pointer items-center gap-2 rounded-full px-5 py-2 text-[12.5px] font-semibold disabled:cursor-not-allowed disabled:opacity-55"
            >
              {exportingKey === "manual" ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  <span>Export en cours…</span>
                </>
              ) : (
                <>
                  <Upload className="h-4 w-4" />
                  <span>Enregistrer &amp; Exporter</span>
                </>
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
