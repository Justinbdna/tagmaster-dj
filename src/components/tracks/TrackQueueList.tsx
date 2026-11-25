import { useMemo, useState } from "react";
import {
  FileAudio2,
  FolderOpen,
  Loader2,
  Music,
  Pencil,
  Search,
  ShieldAlert,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { toLocalAssetUrl } from "../../services/tauriBridge";
import type { AudioTrackItem, TrackStatus } from "../../types/track";
import { formatFileSize } from "../../utils/format";

interface TrackQueueListProps {
  tracks: AudioTrackItem[];
  onRemoveTrack: (id: string) => void;
  onLookupTrack: (id: string) => void;
  onOpenReviewTrack: (id: string) => void;
  onClearAll: () => void;
  onPickFolder: () => void;
  onPickFiles: () => void;
  isScanning: boolean;
  isRunningBatch: boolean;
}

const STATUS_BADGE: Record<
  TrackStatus,
  { label: string; className: string }
> = {
  pending: {
    label: "En attente",
    className:
      "border-[var(--content-border)] bg-[var(--content-card-bg)] text-secondary",
  },
  processing: {
    label: "Recherche…",
    className: "badge-info",
  },
  completed: {
    label: "Identifié",
    className: "badge-success",
  },
  needs_review: {
    label: "À réviser",
    className: "badge-warning",
  },
  failed: {
    label: "Erreur",
    className: "badge-danger",
  },
};

export function TrackQueueList({
  tracks,
  onRemoveTrack,
  onLookupTrack,
  onOpenReviewTrack,
  onClearAll,
  onPickFolder,
  onPickFiles,
  isScanning,
  isRunningBatch,
}: TrackQueueListProps) {
  const isBusy = isScanning || isRunningBatch;
  const [query, setQuery] = useState("");
  const [selectedExt, setSelectedExt] = useState<string>("ALL");
  const [brokenArtworkUrls, setBrokenArtworkUrls] = useState<Set<string>>(
    () => new Set(),
  );

  const formatCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const t of tracks) {
      counts.set(t.extension, (counts.get(t.extension) ?? 0) + 1);
    }
    return Array.from(counts.entries()).sort((a, b) => b[1] - a[1]);
  }, [tracks]);

  const filteredTracks = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return tracks.filter((track) => {
      const matchesExt =
        selectedExt === "ALL" || track.extension === selectedExt;
      const matchesQuery =
        normalized.length === 0 ||
        track.fileName.toLowerCase().includes(normalized) ||
        track.parentFolder.toLowerCase().includes(normalized) ||
        (track.metadata?.title.toLowerCase().includes(normalized) ?? false) ||
        (track.metadata?.artist.toLowerCase().includes(normalized) ?? false);
      return matchesExt && matchesQuery;
    });
  }, [tracks, query, selectedExt]);

  return (
    <section className="content-surface flex min-h-0 flex-1 flex-col overflow-hidden rounded-[24px]">
      {/* Toolbar inside content surface */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--content-divider)] px-5 py-3.5">
        <div className="flex min-w-[220px] flex-1 items-center gap-2.5 rounded-full border border-[var(--content-border)] bg-[var(--content-card-bg)] px-3.5 py-2">
          <Search className="h-3.5 w-3.5 shrink-0 text-muted" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filtrer par titre, artiste ou dossier…"
            className="w-full bg-transparent text-[12.5px] text-primary placeholder:text-muted focus:outline-none"
          />
          {query && (
            <button
              type="button"
              onClick={() => setQuery("")}
              aria-label="Effacer la recherche"
              className="cursor-pointer text-muted hover:text-primary"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        {/* Format filter chips */}
        <div className="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onClick={() => setSelectedExt("ALL")}
            className={`cursor-pointer rounded-full px-2.5 py-1 text-[11px] font-semibold transition-colors ${
              selectedExt === "ALL"
                ? "bg-sky-500/20 text-sky-400"
                : "bg-[var(--content-card-bg)] text-secondary hover:text-primary"
            }`}
          >
            Tous ({tracks.length})
          </button>
          {formatCounts.map(([ext, count]) => (
            <button
              key={ext}
              type="button"
              onClick={() => setSelectedExt(ext)}
              className={`cursor-pointer rounded-full px-2.5 py-1 text-[11px] font-semibold transition-colors ${
                selectedExt === ext
                  ? "bg-sky-500/20 text-sky-400"
                  : "bg-[var(--content-card-bg)] text-secondary hover:text-primary"
              }`}
            >
              .{ext} ({count})
            </button>
          ))}
        </div>

        {/* Quick queue actions */}
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={onPickFolder}
            disabled={isBusy}
            title="Ajouter un autre dossier"
            className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[11.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            <FolderOpen className="h-3.5 w-3.5 text-sky-400" />
            <span>+ Dossier</span>
          </button>
          <button
            type="button"
            onClick={onPickFiles}
            disabled={isBusy}
            title="Ajouter des fichiers individuels"
            className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[11.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-50"
          >
            <FileAudio2 className="h-3.5 w-3.5 text-indigo-400" />
            <span>+ Fichiers</span>
          </button>
          <button
            type="button"
            onClick={onClearAll}
            disabled={isBusy}
            title="Vider la file d'attente"
            className="flex cursor-pointer items-center gap-1 rounded-full border border-rose-500/25 bg-rose-500/10 px-3 py-1.5 text-[11.5px] font-medium text-rose-400 transition-colors hover:bg-rose-500/20 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Trash2 className="h-3.5 w-3.5" />
            <span>Vider</span>
          </button>
        </div>
      </div>

      {/* Column Headers */}
      <div className="grid grid-cols-[34px_minmax(0,1fr)_72px_78px_106px_96px] items-center gap-3 border-b border-[var(--content-divider)] px-5 py-2 text-[10.5px] font-semibold uppercase tracking-[0.12em] text-muted">
        <span className="text-center">#</span>
        <span>Fichier audio &amp; aperçu métadonnées</span>
        <span>Format</span>
        <span className="text-right">Taille</span>
        <span className="text-center">Statut</span>
        <span className="text-right">Actions</span>
      </div>

      {/* Scrollable Track List (Clean, Crisp Content Layer) */}
      <div className="custom-scrollbar min-h-0 flex-1 overflow-y-auto px-2.5 py-2">
        {filteredTracks.length === 0 ? (
          <div className="flex h-40 flex-col items-center justify-center text-center text-[13px] text-muted">
            Aucune piste ne correspond à ce filtre.
          </div>
        ) : (
          <ul className="flex flex-col gap-1">
            {filteredTracks.map((track, idx) => {
              const badge =
                track.status === "completed" && track.exportedPath
                  ? { label: "Exporté", className: STATUS_BADGE.completed.className }
                  : STATUS_BADGE[track.status];
              const isReviewable =
                track.status === "needs_review" || track.status === "failed";
              const meta = track.metadata;
              return (
                <li
                  key={track.id}
                  className="group grid grid-cols-[34px_minmax(0,1fr)_72px_78px_106px_96px] items-center gap-3 rounded-[14px] px-2.5 py-2.5 transition-colors hover:bg-[var(--content-card-hover)]"
                >
                  <span className="text-center text-[11.5px] font-medium tabular-nums text-muted">
                    {idx + 1}
                  </span>

                  <div className="flex min-w-0 items-center gap-3">
                    {(() => {
                      const artworkSrc = track.localArtworkPath
                        ? toLocalAssetUrl(track.localArtworkPath)
                        : meta?.artworkUrl;
                      const canShowImage =
                        Boolean(artworkSrc) &&
                        !brokenArtworkUrls.has(artworkSrc as string);
                      return canShowImage ? (
                        <img
                          src={artworkSrc}
                          alt={meta?.album ?? track.fileName}
                          onError={() => {
                            if (!artworkSrc) return;
                            setBrokenArtworkUrls((prev) => {
                              if (prev.has(artworkSrc)) return prev;
                              const next = new Set(prev);
                              next.add(artworkSrc);
                              return next;
                            });
                          }}
                          className="h-10 w-10 shrink-0 rounded-[10px] border border-[var(--content-border)] object-cover shadow-sm"
                        />
                      ) : (
                        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-[10px] border border-[var(--content-border)] bg-[var(--content-card-bg)] text-sky-400">
                          <Music className="h-4 w-4" strokeWidth={1.85} />
                        </div>
                      );
                    })()}

                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <p
                          className="truncate text-[13px] font-medium text-primary"
                          title={track.sourcePath}
                        >
                          {meta ? `${meta.artist} — ${meta.title}` : track.fileName}
                        </p>
                        {track.hasProtectedKeyword && (
                          <span
                            title="Mot-clé intouchable détecté (Live, Remix, Acoustic, Sped Up, Chopped, Instrumental, Mashup) — protégé contre un écrasement studio Apple Music"
                            className="inline-flex shrink-0 items-center gap-1 rounded-full border badge-warning px-2 py-0.5 text-[10px] font-semibold"
                          >
                            <ShieldAlert className="h-3 w-3" />
                            Version protégée
                          </span>
                        )}
                      </div>

                      {meta ? (
                        <p className="truncate text-[11px] text-secondary">
                          {meta.album}
                          {meta.year ? ` (${meta.year})` : ""}
                          {meta.genre ? ` · ${meta.genre}` : ""}
                          {` · Album Artist : ${meta.albumArtist}`}
                          {meta.discTotal && meta.discTotal > 1
                            ? ` · Disque ${meta.discNumber ?? 1}/${meta.discTotal}`
                            : ""}
                        </p>
                      ) : track.reviewReason ? (
                        <p
                          className="truncate text-[11px] text-amber-400/90"
                          title={track.reviewReason}
                        >
                          {track.reviewReason}
                        </p>
                      ) : (
                        <p className="truncate text-[11px] text-muted">
                          {track.parentFolder}
                        </p>
                      )}
                    </div>
                  </div>

                  <span className="w-fit rounded-md border border-[var(--content-border)] bg-[var(--content-card-bg)] px-2 py-0.5 text-[10.5px] font-semibold tracking-wider text-secondary">
                    {track.extension}
                  </span>

                  <span className="text-right text-[12px] tabular-nums text-secondary">
                    {formatFileSize(track.sizeBytes)}
                  </span>

                  <div className="flex justify-center">
                    {isReviewable ? (
                      <button
                        type="button"
                        onClick={() => onOpenReviewTrack(track.id)}
                        title="Ouvrir dans la File de révision"
                        className={`cursor-pointer rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold transition-transform hover:scale-105 ${badge.className}`}
                      >
                        {badge.label}
                      </button>
                    ) : (
                      <span
                        title={track.exportedPath ?? track.reviewReason}
                        className={`rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold ${badge.className}`}
                      >
                        {badge.label}
                      </span>
                    )}
                  </div>

                  <div className="flex items-center justify-end gap-1">
                    <button
                      type="button"
                      onClick={() => onOpenReviewTrack(track.id)}
                      disabled={isRunningBatch || track.status === "processing"}
                      title="Éditer les métadonnées ou rechercher une autre version"
                      aria-label={`Éditer ${track.fileName}`}
                      className={`flex h-7 w-7 cursor-pointer items-center justify-center rounded-full border border-[var(--content-border)] bg-[var(--content-card-bg)] transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                        isReviewable
                          ? "text-amber-400 hover:bg-amber-500/20"
                          : "text-secondary hover:bg-white/10 hover:text-primary"
                      }`}
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>

                    <button
                      type="button"
                      onClick={() => onLookupTrack(track.id)}
                      disabled={isRunningBatch || track.status === "processing"}
                      title="Tester l'identification Apple Music (US / UK / FR) — lecture seule, sans modifier le fichier"
                      aria-label={`Identifier ${track.fileName} via Apple Music`}
                      className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-full border border-[var(--content-border)] bg-[var(--content-card-bg)] text-sky-400 transition-colors hover:bg-sky-500/20 disabled:cursor-wait disabled:opacity-50"
                    >
                      {track.status === "processing" ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <Sparkles className="h-3.5 w-3.5" />
                      )}
                    </button>

                    <button
                      type="button"
                      onClick={() => onRemoveTrack(track.id)}
                      disabled={isRunningBatch}
                      aria-label={`Retirer ${track.fileName}`}
                      className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-full text-muted opacity-60 transition-all hover:bg-rose-500/15 hover:text-rose-400 hover:opacity-100 disabled:cursor-not-allowed disabled:opacity-30"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
