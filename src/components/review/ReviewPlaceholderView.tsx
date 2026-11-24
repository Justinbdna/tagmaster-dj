import { AlertTriangle, ListChecks, ShieldAlert } from "lucide-react";
import type { AudioTrackItem } from "../../types/track";

interface ReviewPlaceholderViewProps {
  reviewTracks: AudioTrackItem[];
}

export function ReviewPlaceholderView({
  reviewTracks,
}: ReviewPlaceholderViewProps) {
  if (reviewTracks.length === 0) {
    return (
      <section className="content-surface flex flex-1 flex-col items-center justify-center rounded-[24px] p-8 text-center">
        <div className="liquid-glass mb-5 flex h-16 w-16 items-center justify-center rounded-[20px]">
          <ListChecks
            className="relative z-10 h-7 w-7 text-sky-400"
            strokeWidth={1.85}
          />
        </div>
        <h3 className="text-[19px] font-semibold tracking-tight text-primary">
          File de révision post-traitement
        </h3>
        <p className="mt-2 max-w-md text-[13px] leading-relaxed text-secondary">
          Les pistes au titre cryptique, sans pochette ou en conflit de version
          (Live, Remix, Sped Up…) apparaîtront ici sans jamais bloquer vos lots.
          L&apos;édition manuelle et le lecteur comparateur A/B arriveront au
          Sprint 2.
        </p>
      </section>
    );
  }

  return (
    <section className="content-surface flex min-h-0 flex-1 flex-col overflow-hidden rounded-[24px]">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--content-divider)] px-5 py-4">
        <div>
          <h3 className="text-[14.5px] font-semibold text-primary">
            {reviewTracks.length} piste{reviewTracks.length > 1 ? "s" : ""} à
            vérifier manuellement
          </h3>
          <p className="mt-0.5 text-[12px] text-secondary">
            Ces fichiers ont été écartés automatiquement sans bloquer le reste
            du lot.
          </p>
        </div>

        <span className="rounded-full border badge-info px-3 py-1 text-[11px] font-medium">
          Édition manuelle &amp; comparateur A/B : Sprint 2
        </span>
      </div>

      <div className="custom-scrollbar min-h-0 flex-1 overflow-y-auto p-3.5">
        <ul className="flex flex-col gap-2">
          {reviewTracks.map((track) => {
            const isFailed = track.status === "failed";
            return (
              <li
                key={track.id}
                className="flex items-start justify-between gap-4 rounded-[16px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4"
              >
                <div className="flex min-w-0 items-start gap-3">
                  <div
                    className={`mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-[10px] border ${
                      isFailed ? "badge-danger" : "badge-warning"
                    }`}
                  >
                    {isFailed ? (
                      <AlertTriangle className="h-4 w-4" />
                    ) : (
                      <ShieldAlert className="h-4 w-4" />
                    )}
                  </div>

                  <div className="min-w-0">
                    <p
                      className="truncate text-[13.5px] font-semibold text-primary"
                      title={track.sourcePath}
                    >
                      {track.fileName}
                    </p>
                    <p className="mt-0.5 truncate text-[11.5px] text-muted">
                      {track.parentFolder} · .{track.extension}
                    </p>
                    {track.reviewReason && (
                      <p className="mt-1.5 text-[12px] leading-relaxed text-secondary">
                        {track.reviewReason}
                      </p>
                    )}
                  </div>
                </div>

                <span
                  className={`shrink-0 rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold ${
                    isFailed ? "badge-danger" : "badge-warning"
                  }`}
                >
                  {isFailed ? "Erreur" : "À réviser"}
                </span>
              </li>
            );
          })}
        </ul>
      </div>
    </section>
  );
}
