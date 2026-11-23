import { GlassShell } from "../components/layout/GlassShell";
import { useAppHealth } from "../hooks/useAppHealth";

export function WorkspacePage() {
  const { health, isTauriRuntime } = useAppHealth();

  return (
    <GlassShell>
      <header className="flex flex-wrap items-center justify-between gap-4 border-b border-white/10 pb-6">
        <div>
          <p className="text-xs font-medium uppercase tracking-[0.2em] text-sky-300/80">
            Local-First Music Library Engine
          </p>
          <h1 className="mt-1 text-3xl font-semibold tracking-tight text-white">
            Tagmaster DJ
          </h1>
        </div>

        <span className="glass-pill inline-flex items-center gap-2 rounded-full px-4 py-1.5 text-xs font-medium text-white/85">
          <span
            className={`h-2 w-2 rounded-full ${
              isTauriRuntime ? "bg-emerald-400" : "bg-amber-400"
            }`}
          />
          {health
            ? `Backend Rust prêt · v${health.version}`
            : "Aperçu Web · Squelette Sprint 1"}
        </span>
      </header>

      <section className="mt-8 grid gap-4 md:grid-cols-3">
        <div className="glass-pill rounded-2xl p-5">
          <h2 className="text-sm font-semibold text-white">
            1. Import & Nettoyage
          </h2>
          <p className="mt-2 text-xs leading-relaxed text-white/65">
            Sélection de dossier, glisser-déposer et nettoyage intégral « Table
            Rase » des anciens tags.
          </p>
        </div>

        <div className="glass-pill rounded-2xl p-5">
          <h2 className="text-sm font-semibold text-white">
            2. Cascade Apple Music US
          </h2>
          <p className="mt-2 text-xs leading-relaxed text-white/65">
            Enrichissement ID3, featurings normalisés, verrouillage Album Artist
            et pochettes HD.
          </p>
        </div>

        <div className="glass-pill rounded-2xl p-5">
          <h2 className="text-sm font-semibold text-white">
            3. Export Sécurisé
          </h2>
          <p className="mt-2 text-xs leading-relaxed text-white/65">
            Mode Plat ou Dynamique dans{" "}
            <code className="text-sky-300">Tagmaster_DJ_Export</code> sans
            jamais altérer les originaux.
          </p>
        </div>
      </section>
    </GlassShell>
  );
}
