import { useEffect, useState } from "react";
import {
  Check,
  ChevronDown,
  Cpu,
  Disc,
  Eye,
  EyeOff,
  FolderTree,
  Gamepad2,
  HelpCircle,
  KeyRound,
  Layers,
  Loader2,
  Moon,
  ShieldCheck,
  Smartphone,
  Sparkles,
  Sun,
  Trash2,
} from "lucide-react";
import {
  checkFfmpegStatus,
  getApiKeysSettings,
  isTauriEnvironment,
  saveGeniusApiKey,
} from "../../services/tauriBridge";
import type {
  ExportFormatMode,
  ExportMode,
  FfmpegStatus,
  GlassTint,
  HardwareProfile,
  ThemeMode,
} from "../../types/track";

interface SettingsViewProps {
  theme: ThemeMode;
  onSelectTheme: (theme: ThemeMode) => void;
  glassTint: GlassTint;
  onSelectGlassTint: (tint: GlassTint) => void;
  exportMode: ExportMode;
  onSelectExportMode: (mode: ExportMode) => void;
  formatMode: ExportFormatMode;
  onSelectFormatMode: (mode: ExportFormatMode) => void;
  hardwareProfile: HardwareProfile;
  onSelectHardwareProfile: (profile: HardwareProfile) => void;
}

const HARDWARE_PROFILES: {
  id: HardwareProfile;
  label: string;
  spec: string;
  icon: typeof Smartphone;
}[] = [
  {
    id: "standard",
    label: "Standard",
    spec: "1000 px · SYLT/LRC · UTF-8",
    icon: Smartphone,
  },
  {
    id: "ipod",
    label: "iPod",
    spec: "600 px JPEG · FAT32 · ID3v2.3",
    icon: Disc,
  },
  {
    id: "psp",
    label: "PSP 3000",
    spec: "MP3 320k · 400 px · ASCII strict",
    icon: Gamepad2,
  },
];

export function SettingsView({
  theme,
  onSelectTheme,
  glassTint,
  onSelectGlassTint,
  exportMode,
  onSelectExportMode,
  formatMode,
  onSelectFormatMode,
  hardwareProfile,
  onSelectHardwareProfile,
}: SettingsViewProps) {
  const [geniusKey, setGeniusKey] = useState("");
  const [hasGeniusKey, setHasGeniusKey] = useState(false);
  const [envFilePath, setEnvFilePath] = useState(".env");
  const [showGeniusKey, setShowGeniusKey] = useState(false);
  const [showGeniusGuide, setShowGeniusGuide] = useState(false);
  const [isLoadingKeys, setIsLoadingKeys] = useState(false);
  const [isSavingKeys, setIsSavingKeys] = useState(false);
  const [ffmpegStatus, setFfmpegStatus] = useState<FfmpegStatus | null>(null);
  const [saveFeedback, setSaveFeedback] = useState<{
    kind: "success" | "error";
    text: string;
  } | null>(null);

  useEffect(() => {
    if (!isTauriEnvironment()) return;
    let active = true;
    setIsLoadingKeys(true);

    checkFfmpegStatus()
      .then((status) => {
        if (active) setFfmpegStatus(status);
      })
      .catch(() => {
        if (active) setFfmpegStatus({ available: false });
      });

    getApiKeysSettings()
      .then((settings) => {
        if (!active) return;
        setGeniusKey(settings.geniusApiKey);
        setHasGeniusKey(settings.hasGeniusApiKey);
        if (settings.envFilePath) {
          setEnvFilePath(settings.envFilePath);
        }
      })
      .catch(() => {
        // Ignore initial load error in non-desktop or restricted state
      })
      .finally(() => {
        if (active) setIsLoadingKeys(false);
      });

    return () => {
      active = false;
    };
  }, []);

  const persistGeniusKey = async (nextValue: string) => {
    if (!isTauriEnvironment()) {
      setSaveFeedback({
        kind: "error",
        text: "La sauvegarde dans .env nécessite l'application desktop Tauri.",
      });
      return;
    }

    setSaveFeedback(null);
    setIsSavingKeys(true);
    try {
      const updated = await saveGeniusApiKey(nextValue);
      setGeniusKey(updated.geniusApiKey);
      setHasGeniusKey(updated.hasGeniusApiKey);
      if (updated.envFilePath) {
        setEnvFilePath(updated.envFilePath);
      }
      setSaveFeedback({
        kind: "success",
        text: updated.hasGeniusApiKey
          ? "Clé GENIUS_API_KEY enregistrée dans votre .env local."
          : "Clé GENIUS_API_KEY effacée du fichier .env local.",
      });
    } catch (err) {
      setSaveFeedback({
        kind: "error",
        text:
          err instanceof Error
            ? err.message
            : "Impossible d'enregistrer GENIUS_API_KEY dans .env.",
      });
    } finally {
      setIsSavingKeys(false);
    }
  };

  const handleSaveSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isSavingKeys) return;
    void persistGeniusKey(geniusKey.trim());
  };

  return (
    <section className="content-surface custom-scrollbar flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto rounded-[24px] p-5">
      {/* Appearance Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
        <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
          <Sparkles className="h-4 w-4 text-sky-400" />
          <h3>Apparence Liquid Glass</h3>
        </div>
        <p className="mt-0.5 text-[12px] text-secondary">
          Thème clair/sombre et niveau de transparence du verre.
        </p>

        <div className="mt-3.5 grid gap-3.5 md:grid-cols-2">
          <div className="flex flex-col gap-1.5">
            <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">
              Thème
            </span>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onSelectTheme("dark")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[12px] px-3.5 py-2.5 text-[12.5px] font-medium transition-all ${
                  theme === "dark"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <Moon className="h-4 w-4 text-sky-400" />
                <span>Sombre</span>
              </button>
              <button
                type="button"
                onClick={() => onSelectTheme("light")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[12px] px-3.5 py-2.5 text-[12.5px] font-medium transition-all ${
                  theme === "light"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <Sun className="h-4 w-4 text-amber-400" />
                <span>Clair</span>
              </button>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">
              Densité du verre
            </span>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onSelectGlassTint("clear")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[12px] px-3.5 py-2.5 text-[12.5px] font-medium transition-all ${
                  glassTint === "clear"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <span>Translucide</span>
              </button>
              <button
                type="button"
                onClick={() => onSelectGlassTint("tinted")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[12px] px-3.5 py-2.5 text-[12.5px] font-medium transition-all ${
                  glassTint === "tinted"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <span>Teinté (contraste +)</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Export Architecture Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
            <ShieldCheck className="h-4 w-4 text-emerald-400" />
            <h3>Export non destructif</h3>
          </div>
          <code className="rounded-md bg-[var(--content-bg)] px-2 py-0.5 text-[11px] text-sky-400">
            Documents/Tagmaster_DJ_Export
          </code>
        </div>

        <div className="mt-3 grid gap-2.5 md:grid-cols-2">
          <button
            type="button"
            onClick={() => onSelectExportMode("dynamic")}
            className={`flex cursor-pointer flex-col items-start rounded-[14px] px-3.5 py-3 text-left transition-all ${
              exportMode === "dynamic"
                ? "liquid-glass-active"
                : "border border-[var(--content-border)] hover:bg-[var(--content-card-hover)]"
            }`}
          >
            <div className="flex items-center gap-2 text-[12.5px] font-semibold text-primary">
              <FolderTree className="h-3.5 w-3.5 text-sky-400" />
              <span>Dynamique</span>
            </div>
            <p className="mt-1 font-mono text-[11px] text-secondary">
              Artiste / Album / Artiste - Titre.mp3
            </p>
          </button>

          <button
            type="button"
            onClick={() => onSelectExportMode("flat")}
            className={`flex cursor-pointer flex-col items-start rounded-[14px] px-3.5 py-3 text-left transition-all ${
              exportMode === "flat"
                ? "liquid-glass-active"
                : "border border-[var(--content-border)] hover:bg-[var(--content-card-hover)]"
            }`}
          >
            <div className="flex items-center gap-2 text-[12.5px] font-semibold text-primary">
              <Layers className="h-3.5 w-3.5 text-indigo-400" />
              <span>Plat</span>
            </div>
            <p className="mt-1 font-mono text-[11px] text-secondary">
              Artiste - Titre.mp3
            </p>
          </button>
        </div>
      </div>

      {/* Hardware Profile & FFmpeg Audio Mode Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
            <Cpu className="h-4 w-4 text-sky-400" />
            <h3>Profil matériel &amp; Audio</h3>
          </div>
          {ffmpegStatus && (
            <span
              title={ffmpegStatus.version ?? "FFmpeg absent du PATH"}
              className={`rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold ${
                ffmpegStatus.available ? "badge-success" : "badge-warning"
              }`}
            >
              {ffmpegStatus.available ? "FFmpeg prêt" : "FFmpeg absent"}
            </span>
          )}
        </div>

        <div className="mt-3 grid gap-2.5 md:grid-cols-3">
          {HARDWARE_PROFILES.map((item) => {
            const Icon = item.icon;
            const isSelected = hardwareProfile === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onSelectHardwareProfile(item.id)}
                className={`flex cursor-pointer flex-col items-start rounded-[14px] px-3.5 py-3 text-left transition-all ${
                  isSelected
                    ? "liquid-glass-active"
                    : "border border-[var(--content-border)] hover:bg-[var(--content-card-hover)]"
                }`}
              >
                <div className="flex items-center gap-2 text-[12.5px] font-semibold text-primary">
                  <Icon
                    className={`h-3.5 w-3.5 ${
                      isSelected ? "text-semantic-info" : "text-muted"
                    }`}
                  />
                  <span>{item.label}</span>
                </div>
                <p className="mt-1 text-[11px] text-secondary">{item.spec}</p>
              </button>
            );
          })}
        </div>

        <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-[var(--content-divider)] pt-3">
          <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">
            Format audio
          </span>
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              disabled={hardwareProfile === "psp"}
              onClick={() => onSelectFormatMode("passthrough")}
              className={`cursor-pointer rounded-full px-3 py-1.5 text-[11.5px] font-medium transition-all disabled:cursor-not-allowed disabled:opacity-40 ${
                hardwareProfile !== "psp" && formatMode === "passthrough"
                  ? "liquid-glass-active text-primary"
                  : "border border-[var(--content-border)] text-secondary hover:text-primary"
              }`}
            >
              Passthrough
            </button>
            <button
              type="button"
              onClick={() => onSelectFormatMode("force_mp3")}
              className={`cursor-pointer rounded-full px-3 py-1.5 text-[11.5px] font-medium transition-all ${
                hardwareProfile === "psp" || formatMode === "force_mp3"
                  ? "liquid-glass-active text-primary"
                  : "border border-[var(--content-border)] text-secondary hover:text-primary"
              }`}
            >
              Force MP3 320 kbps
            </button>
          </div>
        </div>
      </div>

      {/* API Keys & Lyrics Section */}
      <form
        onSubmit={handleSaveSubmit}
        className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4"
      >
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
            <KeyRound className="h-4 w-4 text-amber-400" />
            <h3>Paroles &amp; Clés API (.env local)</h3>
          </div>
          <span
            className={`rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold ${
              hasGeniusKey
                ? "badge-success"
                : "border-[var(--content-border)] bg-[var(--content-bg)] text-muted"
            }`}
          >
            {hasGeniusKey ? "Genius configurée" : "Optionnelle"}
          </span>
        </div>

        <p className="mt-1 text-[12px] leading-relaxed text-secondary">
          Apple Music, <code className="text-primary">yt-dlp</code> et les
          paroles synchronisées (<code className="text-primary">SYLT</code> +{" "}
          <code className="text-primary">.lrc</code>) fonctionnent sans clé.{" "}
          <code className="text-primary">GENIUS_API_KEY</code> complète en
          option (ignorée sur les{" "}
          <code className="text-primary">Instrumental</code>).
        </p>

        <div className="mt-3.5 flex flex-col gap-2">
          <label
            htmlFor="genius-api-key-input"
            className="text-[11px] font-semibold uppercase tracking-wider text-muted"
          >
            GENIUS_API_KEY
          </label>

          <div className="flex flex-wrap items-center gap-2">
            <div className="relative flex min-w-[220px] flex-1 items-center">
              <input
                id="genius-api-key-input"
                type={showGeniusKey ? "text" : "password"}
                autoComplete="off"
                spellCheck={false}
                disabled={isLoadingKeys || isSavingKeys}
                value={geniusKey}
                onChange={(e) => {
                  setSaveFeedback(null);
                  setGeniusKey(e.target.value);
                }}
                placeholder="Collez votre Client Access Token Genius (optionnel)"
                className="w-full rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] py-2 pl-3.5 pr-10 text-[12.5px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none disabled:opacity-60"
              />
              <button
                type="button"
                onClick={() => setShowGeniusKey((prev) => !prev)}
                aria-label={
                  showGeniusKey
                    ? "Masquer la clé GENIUS_API_KEY"
                    : "Afficher la clé GENIUS_API_KEY"
                }
                title={showGeniusKey ? "Masquer" : "Afficher"}
                className="absolute right-2.5 flex h-7 w-7 cursor-pointer items-center justify-center rounded-full text-muted transition-colors hover:text-primary"
              >
                {showGeniusKey ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </button>
            </div>

            <button
              type="submit"
              disabled={isLoadingKeys || isSavingKeys}
              className="liquid-glass-primary flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12px] font-semibold disabled:cursor-not-allowed disabled:opacity-55"
            >
              {isSavingKeys ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Check className="h-3.5 w-3.5" />
              )}
              <span>Enregistrer</span>
            </button>

            {(hasGeniusKey || geniusKey.trim().length > 0) && (
              <button
                type="button"
                disabled={isLoadingKeys || isSavingKeys}
                onClick={() => void persistGeniusKey("")}
                className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-2 text-[12px] font-medium text-secondary hover:text-rose-400 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <Trash2 className="h-3.5 w-3.5" />
                <span>Effacer</span>
              </button>
            )}
          </div>

          <p className="text-[11px] leading-relaxed text-muted">
            Stockage local uniquement :{" "}
            <code className="break-all text-secondary">{envFilePath}</code>{" "}
            (exclu de Git).
          </p>

          <div className="pt-0.5">
            <button
              type="button"
              onClick={() => setShowGeniusGuide((prev) => !prev)}
              aria-expanded={showGeniusGuide}
              className="inline-flex cursor-pointer items-center gap-1.5 text-[11.5px] font-medium text-sky-400 hover:underline"
            >
              <HelpCircle className="h-3.5 w-3.5" />
              <span>Comment obtenir une clé Genius ?</span>
              <ChevronDown
                className={`h-3.5 w-3.5 transition-transform ${
                  showGeniusGuide ? "rotate-180" : ""
                }`}
              />
            </button>

            {showGeniusGuide && (
              <ol className="mt-2 list-inside list-decimal space-y-1 rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3.5 py-2.5 text-[11.5px] leading-relaxed text-secondary">
                <li>
                  Ouvrir{" "}
                  <code className="text-primary">genius.com/api-clients</code>{" "}
                  et se connecter.
                </li>
                <li>
                  Cliquer sur <strong className="text-primary">New API Client</strong>{" "}
                  (nom libre et URL <code className="text-primary">http://localhost</code>).
                </li>
                <li>
                  Cliquer sur{" "}
                  <strong className="text-primary">Generate Access Token</strong>{" "}
                  et coller le <em>Client Access Token</em> dans le champ
                  ci-dessus.
                </li>
              </ol>
            )}
          </div>
        </div>

        {saveFeedback && (
          <div
            className={`mt-3 rounded-[12px] border px-3.5 py-2 text-[12px] ${
              saveFeedback.kind === "success" ? "badge-success" : "badge-danger"
            }`}
          >
            {saveFeedback.text}
          </div>
        )}
      </form>
    </section>
  );
}
