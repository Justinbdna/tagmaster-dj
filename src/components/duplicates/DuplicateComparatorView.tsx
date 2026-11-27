import { useEffect, useMemo, useState } from "react";
import {
  Check,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Copy,
  Folder,
  Music,
  Sparkles,
  Trash2,
  Users,
} from "lucide-react";
import { makeDuplicatePairKey } from "../../hooks/useTrackLibrary";
import { toLocalAssetUrl } from "../../services/tauriBridge";
import type { AudioTrackItem, DuplicateGroup } from "../../types/track";
import { formatFileSize } from "../../utils/format";
import { LocalAudioPlayer } from "../review/LocalAudioPlayer";

interface DuplicateComparatorViewProps {
  groups: DuplicateGroup[];
  tracks: AudioTrackItem[];
  selectedGroupKey: string | null;
  dismissedPairs: ReadonlySet<string>;
  onSelectGroup: (groupKey: string) => void;
  onKeepTrack: (keepId: string, dropId: string, keptLabel: string) => void;
  onKeepBoth: (idA: string, idB: string) => void;
  onDeleteBoth: (idA: string, idB: string) => void;
  onBackToLibrary: () => void;
}

function qualityScore(track: AudioTrackItem): number {
  const ext = track.extension.toUpperCase();
  const formatWeight =
    ext === "FLAC" || ext === "WAV"
      ? 4_000_000_000
      : ext === "M4A" || ext === "AAC"
        ? 3_000_000_000
        : 2_000_000_000;
  const bitrateWeight = (track.bitrateKbps ?? 0) * 1_000_000;
  return formatWeight + bitrateWeight + track.sizeBytes;
}

function formatSampleRate(hz?: number): string {
  if (!hz || hz <= 0) return "";
  return `${(hz / 1000).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} kHz`;
}

function TrackSideCard({
  sideLabel,
  accentClass,
  track,
  isRecommended,
  externalPauseSignal,
  onPlayStart,
}: {
  sideLabel: "A" | "B";
  accentClass: string;
  track: AudioTrackItem;
  isRecommended: boolean;
  externalPauseSignal: number;
  onPlayStart: () => void;
}) {
  const [artworkBroken, setArtworkBroken] = useState(false);
  const artworkSrc = track.localArtworkPath
    ? toLocalAssetUrl(track.localArtworkPath)
    : track.metadata?.artworkUrl;

  useEffect(() => {
    setArtworkBroken(false);
  }, [track.id, artworkSrc]);

  const displayTitle = track.metadata
    ? `${track.metadata.artist} — ${track.metadata.title}`
    : track.fileName;
  const showFileNameSubline = displayTitle !== track.fileName;
  const isLossless =
    track.extension === "FLAC" || track.extension === "WAV";

  const bitrateParts: string[] = [];
  if (track.bitrateKbps) bitrateParts.push(`${track.bitrateKbps} kbps`);
  const sr = formatSampleRate(track.sampleRateHz);
  if (sr) bitrateParts.push(sr);
  const bitrateLabel = bitrateParts.length > 0 ? bitrateParts.join(" · ") : "—";

  return (
    <article className="comparator-card flex min-h-0 flex-col justify-between gap-4 rounded-[22px] p-5">
      <div className="flex flex-col gap-4">
        {/* Card header: Side badge + optional Lossless + Quality recommendation */}
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <span
              className={`rounded-full border px-2.5 py-0.5 text-[11px] font-bold tracking-wider ${accentClass}`}
            >
              PISTE {sideLabel}
            </span>
            {isLossless && (
              <span className="badge-info rounded-full border px-2 py-0.5 text-[10.5px] font-semibold">
                Lossless
              </span>
            )}
          </div>

          {isRecommended && (
            <span className="badge-success inline-flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold">
              <Sparkles className="h-3 w-3" />
              Qualité recommandée
            </span>
          )}
        </div>

        {/* Cover + Track Identity (deduplicated) */}
        <div className="flex items-center gap-3.5">
          {artworkSrc && !artworkBroken ? (
            <img
              src={artworkSrc}
              alt={displayTitle}
              onError={() => setArtworkBroken(true)}
              className="h-14 w-14 shrink-0 rounded-[13px] border border-[var(--content-border)] object-cover shadow-sm"
            />
          ) : (
            <div className="modal-subcard text-semantic-info flex h-14 w-14 shrink-0 items-center justify-center rounded-[13px]">
              <Music className="h-5 w-5" strokeWidth={1.85} />
            </div>
          )}

          <div className="min-w-0 flex-1">
            <h3
              className="truncate text-[15px] font-semibold text-primary"
              title={displayTitle}
            >
              {displayTitle}
            </h3>
            {showFileNameSubline && (
              <p
                className="mt-0.5 truncate font-mono text-[11.5px] text-secondary"
                title={track.fileName}
              >
                {track.fileName}
              </p>
            )}
            {track.metadata && (
              <p className="mt-0.5 truncate text-[11.5px] text-muted">
                {track.metadata.album}
                {track.metadata.year ? ` (${track.metadata.year})` : ""}
                {track.metadata.genre ? ` · ${track.metadata.genre}` : ""}
              </p>
            )}
          </div>
        </div>

        {/* Local Audio Player with mutual auto-stop (includes live duration) */}
        <LocalAudioPlayer
          sourcePath={track.sourcePath}
          fileName={track.fileName}
          externalPauseSignal={externalPauseSignal}
          onPlayStart={onPlayStart}
        />

        {/* Compact High-Contrast Technical Specs Strip */}
        <div className="modal-subcard flex flex-wrap items-center justify-between gap-x-4 gap-y-2 rounded-[14px] px-4 py-3 text-[12px]">
          <div className="flex items-baseline gap-1.5">
            <span className="text-[10.5px] font-semibold uppercase tracking-wider text-muted">
              Format
            </span>
            <span className="font-semibold text-primary">{track.extension}</span>
          </div>

          <div className="flex items-baseline gap-1.5">
            <span className="text-[10.5px] font-semibold uppercase tracking-wider text-muted">
              Audio
            </span>
            <span className="font-semibold tabular-nums text-primary">
              {bitrateLabel}
            </span>
          </div>

          <div className="flex items-baseline gap-1.5">
            <span className="text-[10.5px] font-semibold uppercase tracking-wider text-muted">
              Taille
            </span>
            <span className="font-semibold tabular-nums text-primary">
              {formatFileSize(track.sizeBytes)}
            </span>
          </div>

          <div
            className="flex min-w-0 items-center gap-1.5"
            title={track.sourcePath}
          >
            <Folder className="text-semantic-warning h-3.5 w-3.5 shrink-0" />
            <span className="max-w-[180px] truncate font-semibold text-primary">
              {track.parentFolder}
            </span>
          </div>
        </div>
      </div>

      {/* Discreet readable source path */}
      <p
        className="truncate font-mono text-[11px] text-secondary"
        title={track.sourcePath}
      >
        {track.sourcePath}
      </p>
    </article>
  );
}

export function DuplicateComparatorView({
  groups,
  tracks,
  selectedGroupKey,
  dismissedPairs,
  onSelectGroup,
  onKeepTrack,
  onKeepBoth,
  onDeleteBoth,
  onBackToLibrary,
}: DuplicateComparatorViewProps) {
  const [pauseSignalA, setPauseSignalA] = useState(0);
  const [pauseSignalB, setPauseSignalB] = useState(0);
  const [manualPairIds, setManualPairIds] = useState<[string, string] | null>(
    null,
  );

  const trackById = useMemo(
    () => new Map(tracks.map((t) => [t.id, t])),
    [tracks],
  );

  const activeGroupIndex = useMemo(() => {
    if (groups.length === 0) return -1;
    if (!selectedGroupKey) return 0;
    const found = groups.findIndex((g) => g.groupKey === selectedGroupKey);
    return found >= 0 ? found : 0;
  }, [groups, selectedGroupKey]);

  const activeGroup = activeGroupIndex >= 0 ? groups[activeGroupIndex] : null;

  const groupTracks = useMemo(() => {
    if (!activeGroup) return [];
    return activeGroup.trackIds
      .map((id) => trackById.get(id))
      .filter((t): t is AudioTrackItem => Boolean(t));
  }, [activeGroup, trackById]);

  useEffect(() => {
    setManualPairIds(null);
    setPauseSignalA((n) => n + 1);
    setPauseSignalB((n) => n + 1);
  }, [activeGroup?.groupKey]);

  const [trackA, trackB] = useMemo<
    [AudioTrackItem | null, AudioTrackItem | null]
  >(() => {
    if (groupTracks.length < 2) return [null, null];

    if (manualPairIds) {
      const a = groupTracks.find((t) => t.id === manualPairIds[0]);
      const b = groupTracks.find((t) => t.id === manualPairIds[1]);
      if (a && b && a.id !== b.id) {
        return [a, b];
      }
    }

    for (let i = 0; i < groupTracks.length; i += 1) {
      for (let j = i + 1; j < groupTracks.length; j += 1) {
        if (
          !dismissedPairs.has(
            makeDuplicatePairKey(groupTracks[i].id, groupTracks[j].id),
          )
        ) {
          return [groupTracks[i], groupTracks[j]];
        }
      }
    }

    return [groupTracks[0], groupTracks[1]];
  }, [groupTracks, manualPairIds, dismissedPairs]);

  if (!activeGroup || !trackA || !trackB) {
    return (
      <section className="content-surface flex min-h-0 flex-1 flex-col items-center justify-center gap-4 rounded-[24px] p-8 text-center">
        <div className="badge-success flex h-14 w-14 items-center justify-center rounded-[18px] border">
          <CheckCircle2 className="h-7 w-7" />
        </div>
        <div>
          <h3 className="text-[17px] font-semibold text-primary">
            Aucun doublon en attente
          </h3>
          <p className="mt-1 text-[13px] text-secondary">
            Toutes les pistes de la file ont été arbitrées.
          </p>
        </div>
        <button
          type="button"
          onClick={onBackToLibrary}
          className="liquid-glass-primary cursor-pointer rounded-full px-5 py-2 text-[12.5px] font-semibold"
        >
          Retour à la Bibliothèque
        </button>
      </section>
    );
  }

  const scoreA = qualityScore(trackA);
  const scoreB = qualityScore(trackB);
  const recommendA = scoreA > scoreB;
  const recommendB = scoreB > scoreA;

  const stopBothPlayers = () => {
    setPauseSignalA((n) => n + 1);
    setPauseSignalB((n) => n + 1);
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-3.5">
      {/* Compact Group Navigator Bar */}
      <div className="liquid-glass flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-[20px] px-4 py-3">
        <div className="relative z-10 flex min-w-0 items-center gap-3">
          <div className="badge-duplicate flex h-9 w-9 shrink-0 items-center justify-center rounded-[12px] border">
            <Copy className="h-4 w-4" />
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-[14.5px] font-semibold text-primary">
              {activeGroup.displayLabel}
            </h3>
            <p className="truncate text-[12px] text-secondary">
              {groupTracks.length} fichiers · Groupe {activeGroupIndex + 1}/
              {groups.length}
            </p>
          </div>
        </div>

        {/* Pair selector (if 3+) + Prev / Next Group Switcher */}
        <div className="relative z-10 flex items-center gap-2">
          {groupTracks.length > 2 && (
            <div className="modal-subcard flex items-center gap-1.5 rounded-full px-3 py-1 text-[11.5px] text-secondary">
              <select
                value={trackA.id}
                onChange={(e) => {
                  const nextA = e.target.value;
                  const fallbackB =
                    trackB.id !== nextA
                      ? trackB.id
                      : (groupTracks.find((t) => t.id !== nextA)?.id ??
                        trackB.id);
                  setManualPairIds([nextA, fallbackB]);
                }}
                aria-label="Sélectionner la piste A"
                className="max-w-[140px] cursor-pointer truncate bg-transparent font-semibold text-primary focus:outline-none"
              >
                {groupTracks.map((t, idx) => (
                  <option key={t.id} value={t.id} className="text-slate-900">
                    #{idx + 1} {t.extension} ({formatFileSize(t.sizeBytes)})
                  </option>
                ))}
              </select>
              <span>vs</span>
              <select
                value={trackB.id}
                onChange={(e) => {
                  const nextB = e.target.value;
                  const fallbackA =
                    trackA.id !== nextB
                      ? trackA.id
                      : (groupTracks.find((t) => t.id !== nextB)?.id ??
                        trackA.id);
                  setManualPairIds([fallbackA, nextB]);
                }}
                aria-label="Sélectionner la piste B"
                className="max-w-[140px] cursor-pointer truncate bg-transparent font-semibold text-primary focus:outline-none"
              >
                {groupTracks.map((t, idx) => (
                  <option key={t.id} value={t.id} className="text-slate-900">
                    #{idx + 1} {t.extension} ({formatFileSize(t.sizeBytes)})
                  </option>
                ))}
              </select>
            </div>
          )}

          {groups.length > 1 && (
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => {
                  const prevIdx =
                    (activeGroupIndex - 1 + groups.length) % groups.length;
                  onSelectGroup(groups[prevIdx].groupKey);
                }}
                aria-label="Groupe précédent"
                className="liquid-glass-pill flex h-8 w-8 cursor-pointer items-center justify-center rounded-full text-secondary hover:text-primary"
              >
                <ChevronLeft className="h-4 w-4" />
              </button>
              <button
                type="button"
                onClick={() => {
                  const nextIdx = (activeGroupIndex + 1) % groups.length;
                  onSelectGroup(groups[nextIdx].groupKey);
                }}
                aria-label="Groupe suivant"
                className="liquid-glass-pill flex h-8 w-8 cursor-pointer items-center justify-center rounded-full text-secondary hover:text-primary"
              >
                <ChevronRight className="h-4 w-4" />
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Side-by-Side A / B Comparison Area */}
      <div className="custom-scrollbar grid min-h-0 flex-1 grid-cols-1 gap-4 overflow-y-auto pr-0.5 lg:grid-cols-2">
        <TrackSideCard
          sideLabel="A"
          accentClass="badge-info"
          track={trackA}
          isRecommended={recommendA}
          externalPauseSignal={pauseSignalA}
          onPlayStart={() => setPauseSignalB((n) => n + 1)}
        />

        <TrackSideCard
          sideLabel="B"
          accentClass="badge-duplicate"
          track={trackB}
          isRecommended={recommendB}
          externalPauseSignal={pauseSignalB}
          onPlayStart={() => setPauseSignalA((n) => n + 1)}
        />
      </div>

      {/* Clean Centered 4-Action Floating Decision Bar */}
      <footer className="liquid-glass flex shrink-0 items-center justify-center rounded-[22px] px-5 py-3.5">
        <div className="relative z-10 flex flex-wrap items-center justify-center gap-2.5">
          <button
            type="button"
            onClick={() => {
              stopBothPlayers();
              onKeepTrack(trackA.id, trackB.id, "Piste A");
            }}
            className="liquid-glass-primary flex cursor-pointer items-center gap-1.5 rounded-full px-5 py-2 text-[12.5px] font-semibold"
          >
            <Check className="h-3.5 w-3.5" strokeWidth={2.4} />
            <span>Garder A</span>
          </button>

          <button
            type="button"
            onClick={() => {
              stopBothPlayers();
              onKeepTrack(trackB.id, trackA.id, "Piste B");
            }}
            className="btn-semantic-accent flex cursor-pointer items-center gap-1.5 rounded-full px-5 py-2 text-[12.5px] font-semibold"
          >
            <Check className="h-3.5 w-3.5" strokeWidth={2.4} />
            <span>Garder B</span>
          </button>

          <button
            type="button"
            onClick={() => {
              stopBothPlayers();
              onKeepBoth(trackA.id, trackB.id);
            }}
            className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12.5px] font-medium text-primary"
          >
            <Users className="text-semantic-success h-3.5 w-3.5" />
            <span>Garder les deux</span>
          </button>

          <button
            type="button"
            onClick={() => {
              stopBothPlayers();
              onDeleteBoth(trackA.id, trackB.id);
            }}
            className="btn-semantic-danger flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12.5px] font-semibold"
          >
            <Trash2 className="h-3.5 w-3.5" />
            <span>Supprimer les deux</span>
          </button>
        </div>
      </footer>
    </section>
  );
}
