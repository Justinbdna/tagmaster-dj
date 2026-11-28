import { useEffect, useRef, useState } from "react";
import {
  Check,
  ChevronDown,
  Cpu,
  Eye,
  EyeOff,
  FolderOpen,
  FolderTree,
  HelpCircle,
  KeyRound,
  Layers,
  Loader2,
  Moon,
  RotateCcw,
  ShieldCheck,
  Sparkles,
  Sun,
  Trash2,
} from "lucide-react";
import {
  checkFfmpegStatus,
  getApiKeysSettings,
  isTauriEnvironment,
  saveDiscogsToken,
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
import {
  HARDWARE_PROFILE_LABELS,
  HardwareProfileIcon,
} from "../ui/HardwareIcons";

interface SettingsViewProps {
  theme: ThemeMode;
  onSelectTheme: (theme: ThemeMode) => void;
  glassTint: GlassTint;
  onSelectGlassTint: (tint: GlassTint) => void;
  exportMode: ExportMode;
  onSelectExportMode: (mode: ExportMode) => void;
  exportDirPath: string;
  isCustomExportDir: boolean;
  onPickExportDir: () => void;
  onResetExportDir: () => void;
  onOpenExportFolder: () => void;
  formatMode: ExportFormatMode;
  onSelectFormatMode: (mode: ExportFormatMode) => void;
  hardwareProfile: HardwareProfile;
  onSelectHardwareProfile: (profile: HardwareProfile) => void;
}

const HARDWARE_PROFILE_INFO: Record<
  HardwareProfile,
  { title: string; bullets: string[] }
> = {
  standard: {
    title: "Standard (Mac / iPhone / DJ)",
    bullets: [
      "Audio conservé au format d'origine",
      "Pochette HD jusqu'à 1000 × 1000 px",
      "Paroles synchronisées SYLT + USLT + .lrc",
      "Tags UTF-8 complets (ID3v2.4)",
    ],
  },
  ipod: {
    title: "iPod Classic / Nano / Touch",
    bullets: [
      "MP3/M4A conservés · FLAC/WAV convertis en MP3 320k",
      "Pochette JPEG baseline max 600 × 600 px",
      "Sanitizer FAT32 (sans emojis ni caractères risqués)",
      "Tags ID3v2.3 compatibles iTunes / iPod",
    ],
  },
  psp: {
    title: "PlayStation Portable (PSP)",
    bullets: [
      "Conversion forcée MP3 CBR 320 kbps · 44,1 kHz",
      "Pochette JPEG baseline max 400 × 400 px",
      "Sanitizer ASCII strict (Memory Stick Pro Duo)",
      "ID3v2.3 épuré pour XMB (sans .lrc)",
    ],
  },
};

const PROFILE_ORDER: HardwareProfile[] = ["standard", "ipod", "psp"];

export function SettingsView({
  theme,
  onSelectTheme,
  glassTint,
  onSelectGlassTint,
  exportMode,
  onSelectExportMode,
  exportDirPath,
  isCustomExportDir,
  onPickExportDir,
  onResetExportDir,
  onOpenExportFolder,
  formatMode,
  onSelectFormatMode,
  hardwareProfile,
  onSelectHardwareProfile,
}: SettingsViewProps) {
  const [geniusKey, setGeniusKey] = useState("");
  const [hasGeniusKey, setHasGeniusKey] = useState(false);
  const [discogsToken, setDiscogsToken] = useState("");
  const [hasDiscogsToken, setHasDiscogsToken] = useState(false);
  const [envFilePath, setEnvFilePath] = useState(".env");
  const [showGeniusKey, setShowGeniusKey] = useState(false);
  const [showDiscogsToken, setShowDiscogsToken] = useState(false);
  const [showGeniusGuide, setShowGeniusGuide] = useState(false);
  const [showDiscogsGuide, setShowDiscogsGuide] = useState(false);
  const [isLoadingKeys, setIsLoadingKeys] = useState(false);
  const [isSavingKeys, setIsSavingKeys] = useState(false);
  const [isSavingDiscogs, setIsSavingDiscogs] = useState(false);
  const [ffmpegStatus, setFfmpegStatus] = useState<FfmpegStatus | null>(null);
  const [hoveredProfile, setHoveredProfile] = useState<HardwareProfile | null>(
    null,
  );
  const hoverTimerRef = useRef<number | null>(null);
  const [saveFeedback, setSaveFeedback] = useState<{
    kind: "success" | "error";
    text: string;
  } | null>(null);

  useEffect(() => {
    return () => {
      if (hoverTimerRef.current !== null) {
        window.clearTimeout(hoverTimerRef.current);
      }
    };
  }, []);

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
        setDiscogsToken(settings.discogsToken);
        setHasDiscogsToken(settings.hasDiscogsToken);
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

  const handleProfileMouseEnter = (profile: HardwareProfile) => {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
    }
    hoverTimerRef.current = window.setTimeout(() => {
      setHoveredProfile(profile);
    }, 200);
  };

  const handleProfileMouseLeave = () => {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    setHoveredProfile(null);
  };

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

  const persistDiscogsToken = async (nextValue: string) => {
    if (!isTauriEnvironment()) {
      setSaveFeedback({
        kind: "error",
        text: "La sauvegarde dans .env nécessite l'application desktop Tauri.",
      });
      return;
    }

    setSaveFeedback(null);
    setIsSavingDiscogs(true);
    try {
      const updated = await saveDiscogsToken(nextValue);
      setDiscogsToken(updated.discogsToken);
      setHasDiscogsToken(updated.hasDiscogsToken);
      if (updated.envFilePath) {
        setEnvFilePath(updated.envFilePath);
      }
      setSaveFeedback({
        kind: "success",
        text: updated.hasDiscogsToken
          ? "Token DISCOGS_TOKEN enregistré dans votre .env local."
          : "Token DISCOGS_TOKEN effacé du fichier .env local.",
      });
    } catch (err) {
      setSaveFeedback({
        kind: "error",
        text:
          err instanceof Error
            ? err.message
            : "Impossible d'enregistrer DISCOGS_TOKEN dans .env.",
      });
    } finally {
      setIsSavingDiscogs(false);
    }
  };

  const handleSaveSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isSavingKeys) return;
    void persistGeniusKey(geniusKey.trim());
  };

  const isMp3Forced = hardwareProfile === "psp" || formatMode === "force_mp3";

  return (
    <section className="content-surface custom-scrollbar flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto rounded-[24px] p-5">
      {/* Appearance Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
        <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
          <Sparkles className="h-4 w-4 text-sky-400" />
          <h3>Apparence Liquid Glass</h3>
        </div>

        <div className="mt-3.5 grid gap-3.5 md:grid-cols-2">
          {/* Theme sliding toggle */}
          <div className="flex flex-col gap-1.5">
            <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">
              Thème
            </span>
            <div className="relative grid grid-cols-2 rounded-[14px] border border-[var(--content-border)] bg-[var(--content-bg)] p-1">
              <span
                aria-hidden="true"
                className="liquid-glass-active pointer-events-none absolute inset-y-1 left-1 w-[calc(50%-4px)] rounded-[10px] transition-transform duration-200 ease-out"
                style={{
                  transform:
                    theme === "light" ? "translateX(100%)" : "translateX(0%)",
                }}
              />
              <button
                type="button"
                onClick={() => onSelectTheme("dark")}
                className={`relative z-10 flex cursor-pointer items-center justify-center gap-2 rounded-[10px] px-3.5 py-2 text-[12.5px] font-medium transition-colors duration-200 ${
                  theme === "dark"
                    ? "text-primary"
                    : "text-secondary hover:text-primary"
                }`}
              >
                <Moon className="h-4 w-4 text-sky-400" />
                <span>Sombre</span>
              </button>
              <button
                type="button"
                onClick={() => onSelectTheme("light")}
                className={`relative z-10 flex cursor-pointer items-center justify-center gap-2 rounded-[10px] px-3.5 py-2 text-[12.5px] font-medium transition-colors duration-200 ${
                  theme === "light"
                    ? "text-primary"
                    : "text-secondary hover:text-primary"
                }`}
              >
                <Sun className="h-4 w-4 text-amber-400" />
                <span>Clair</span>
              </button>
            </div>
          </div>

          {/* Glass density sliding toggle */}
          <div className="flex flex-col gap-1.5">
            <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">
              Densité du verre
            </span>
            <div className="relative grid grid-cols-2 rounded-[14px] border border-[var(--content-border)] bg-[var(--content-bg)] p-1">
              <span
                aria-hidden="true"
                className="liquid-glass-active pointer-events-none absolute inset-y-1 left-1 w-[calc(50%-4px)] rounded-[10px] transition-transform duration-200 ease-out"
                style={{
                  transform:
                    glassTint === "tinted"
                      ? "translateX(100%)"
                      : "translateX(0%)",
                }}
              />
              <button
                type="button"
                onClick={() => onSelectGlassTint("clear")}
                className={`relative z-10 flex cursor-pointer items-center justify-center gap-2 rounded-[10px] px-3.5 py-2 text-[12.5px] font-medium transition-colors duration-200 ${
                  glassTint === "clear"
                    ? "text-primary"
                    : "text-secondary hover:text-primary"
                }`}
              >
                <span>Translucide</span>
              </button>
              <button
                type="button"
                onClick={() => onSelectGlassTint("tinted")}
                className={`relative z-10 flex cursor-pointer items-center justify-center gap-2 rounded-[10px] px-3.5 py-2 text-[12.5px] font-medium transition-colors duration-200 ${
                  glassTint === "tinted"
                    ? "text-primary"
                    : "text-secondary hover:text-primary"
                }`}
              >
                <span>Teinté</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Export Architecture Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
            <ShieldCheck className="text-semantic-success h-4 w-4" />
            <h3>Type d&apos;export</h3>
          </div>
          <div className="flex min-w-0 max-w-full items-center gap-1.5">
            <button
              type="button"
              onClick={onOpenExportFolder}
              title={`Ouvrir le dossier d'export : ${exportDirPath}`}
              className="text-semantic-info max-w-[260px] cursor-pointer truncate rounded-md border border-[var(--content-border)] bg-[var(--content-bg)] px-2 py-0.5 font-mono text-[11px] transition-colors hover:border-sky-400/50 hover:bg-[var(--content-card-hover)] sm:max-w-[340px]"
            >
              {exportDirPath}
            </button>
            <button
              type="button"
              onClick={onPickExportDir}
              title="Choisir un dossier de destination (disque externe, clé USB, PSP…)"
              className="liquid-glass-pill flex shrink-0 cursor-pointer items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium text-primary"
            >
              <FolderOpen className="h-3 w-3 text-sky-400" />
              <span>Changer…</span>
            </button>
            {isCustomExportDir && (
              <button
                type="button"
                onClick={onResetExportDir}
                title="Réinitialiser vers Documents/Tagmaster_DJ_Export"
                className="flex shrink-0 cursor-pointer items-center gap-1 rounded-full border border-[var(--content-border)] bg-[var(--content-bg)] px-2.5 py-1 text-[11px] font-medium text-secondary transition-colors hover:bg-[var(--content-card-hover)] hover:text-primary"
              >
                <RotateCcw className="h-3 w-3" />
                <span>Par défaut</span>
              </button>
            )}
          </div>
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

        {/* 3 Minimalist Hardware Profile Cards with 200ms Liquid Glass Tooltip */}
        <div className="mt-3 grid gap-2.5 md:grid-cols-3">
          {PROFILE_ORDER.map((profileId) => {
            const isSelected = hardwareProfile === profileId;
            const info = HARDWARE_PROFILE_INFO[profileId];
            const showTooltip = hoveredProfile === profileId;

            return (
              <div
                key={profileId}
                className="relative"
                onMouseEnter={() => handleProfileMouseEnter(profileId)}
                onMouseLeave={handleProfileMouseLeave}
              >
                {showTooltip && (
                  <div
                    role="tooltip"
                    className="modal-surface pointer-events-none absolute bottom-full left-1/2 z-40 mb-2 w-64 -translate-x-1/2 rounded-[16px] p-3.5 text-left"
                  >
                    <p className="text-[12px] font-semibold text-primary">
                      {info.title}
                    </p>
                    <ul className="mt-1.5 space-y-1 text-[11px] leading-snug text-secondary">
                      {info.bullets.map((bullet) => (
                        <li key={bullet} className="flex items-start gap-1.5">
                          <span className="text-semantic-info mt-0.5">•</span>
                          <span>{bullet}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

                <button
                  type="button"
                  onClick={() => onSelectHardwareProfile(profileId)}
                  className={`flex w-full cursor-pointer items-center justify-center gap-2.5 rounded-[14px] px-4 py-3 text-[13px] font-semibold transition-all ${
                    isSelected
                      ? "liquid-glass-active text-primary"
                      : "border border-[var(--content-border)] text-secondary hover:bg-[var(--content-card-hover)] hover:text-primary"
                  }`}
                >
                  <HardwareProfileIcon
                    profile={profileId}
                    className={`h-[18px] w-[18px] shrink-0 ${
                      isSelected ? "text-semantic-info" : "text-muted"
                    }`}
                  />
                  <span>{HARDWARE_PROFILE_LABELS[profileId]}</span>
                </button>
              </div>
            );
          })}
        </div>

        {/* Format Audio Sliding Toggle: Original vs MP3 320k */}
        <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-[var(--content-divider)] pt-3">
          <span className="text-[11px] font-semibold uppercase tracking-wider text-muted">
            Format audio
          </span>
          <div className="relative grid grid-cols-2 rounded-full border border-[var(--content-border)] bg-[var(--content-bg)] p-0.5">
            <span
              aria-hidden="true"
              className="liquid-glass-active pointer-events-none absolute inset-y-0.5 left-0.5 w-[calc(50%-2px)] rounded-full transition-transform duration-200 ease-out"
              style={{
                transform: isMp3Forced ? "translateX(100%)" : "translateX(0%)",
              }}
            />
            <button
              type="button"
              disabled={hardwareProfile === "psp"}
              onClick={() => onSelectFormatMode("passthrough")}
              title="Conserve le format audio d'origine sans réencodage (Passthrough)"
              className={`relative z-10 cursor-pointer rounded-full px-3.5 py-1.5 text-[11.5px] font-medium transition-colors duration-200 disabled:cursor-not-allowed disabled:opacity-40 ${
                !isMp3Forced
                  ? "text-primary"
                  : "text-secondary hover:text-primary"
              }`}
            >
              Original
            </button>
            <button
              type="button"
              onClick={() => onSelectFormatMode("force_mp3")}
              title="Convertit chaque piste en MP3 CBR 320 kbps · 44,1 kHz stéréo via FFmpeg"
              className={`relative z-10 cursor-pointer rounded-full px-3.5 py-1.5 text-[11.5px] font-medium transition-colors duration-200 ${
                isMp3Forced
                  ? "text-primary"
                  : "text-secondary hover:text-primary"
              }`}
            >
              MP3 320k
            </button>
          </div>
        </div>
      </div>

      {/* API Keys & Lyrics Section (Clean & Minimalist) */}
      <form
        onSubmit={handleSaveSubmit}
        className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-4"
      >
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
            <KeyRound className="text-semantic-warning h-4 w-4" />
            <h3>Paroles &amp; Clés API</h3>
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
                placeholder="Client Access Token Genius (optionnel)"
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

          <div className="pt-0.5">
            <button
              type="button"
              onClick={() => setShowGeniusGuide((prev) => !prev)}
              aria-expanded={showGeniusGuide}
              className="text-semantic-info inline-flex cursor-pointer items-center gap-1.5 text-[11.5px] font-medium hover:underline"
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
              <div className="mt-2 space-y-1.5 rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3.5 py-2.5 text-[11.5px] leading-relaxed text-secondary">
                <p>
                  Les paroles synchronisées (<code className="text-primary">SYLT</code> +{" "}
                  <code className="text-primary">.lrc</code>) fonctionnent sans
                  clé. Genius sert de complément (ignoré sur les{" "}
                  <code className="text-primary">Instrumental</code>) et est
                  enregistré dans{" "}
                  <code className="break-all text-primary">{envFilePath}</code>.
                </p>
                <ol className="list-inside list-decimal space-y-1">
                  <li>
                    Ouvrir{" "}
                    <code className="text-primary">genius.com/api-clients</code>{" "}
                    et se connecter.
                  </li>
                  <li>
                    Cliquer sur{" "}
                    <strong className="text-primary">New API Client</strong> (nom
                    libre et URL{" "}
                    <code className="text-primary">http://localhost</code>).
                  </li>
                  <li>
                    Cliquer sur{" "}
                    <strong className="text-primary">
                      Generate Access Token
                    </strong>{" "}
                    puis coller le token ci-dessus.
                  </li>
                </ol>
              </div>
            )}
          </div>

          {/* DISCOGS_TOKEN */}
          <div className="mt-3 flex flex-col gap-2 border-t border-[var(--content-divider)] pt-3.5">
            <div className="flex items-center justify-between gap-2">
              <label
                htmlFor="discogs-token-input"
                className="text-[11px] font-semibold uppercase tracking-wider text-muted"
              >
                DISCOGS_TOKEN
              </label>
              <span
                className={`rounded-full border px-2.5 py-0.5 text-[10.5px] font-semibold ${
                  hasDiscogsToken
                    ? "badge-discogs"
                    : "border-[var(--content-border)] bg-[var(--content-bg)] text-muted"
                }`}
              >
                {hasDiscogsToken ? "Discogs configuré" : "Optionnel"}
              </span>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <div className="relative flex min-w-[220px] flex-1 items-center">
                <input
                  id="discogs-token-input"
                  type={showDiscogsToken ? "text" : "password"}
                  autoComplete="off"
                  spellCheck={false}
                  disabled={isLoadingKeys || isSavingDiscogs}
                  value={discogsToken}
                  onChange={(e) => {
                    setSaveFeedback(null);
                    setDiscogsToken(e.target.value);
                  }}
                  placeholder="Personal Access Token Discogs (optionnel)"
                  className="w-full rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] py-2 pl-3.5 pr-10 text-[12.5px] text-primary placeholder:text-muted focus:border-sky-400/60 focus:outline-none disabled:opacity-60"
                />
                <button
                  type="button"
                  onClick={() => setShowDiscogsToken((prev) => !prev)}
                  aria-label={
                    showDiscogsToken
                      ? "Masquer DISCOGS_TOKEN"
                      : "Afficher DISCOGS_TOKEN"
                  }
                  title={showDiscogsToken ? "Masquer" : "Afficher"}
                  className="absolute right-2.5 flex h-7 w-7 cursor-pointer items-center justify-center rounded-full text-muted transition-colors hover:text-primary"
                >
                  {showDiscogsToken ? (
                    <EyeOff className="h-4 w-4" />
                  ) : (
                    <Eye className="h-4 w-4" />
                  )}
                </button>
              </div>

              <button
                type="button"
                disabled={isLoadingKeys || isSavingDiscogs}
                onClick={() => void persistDiscogsToken(discogsToken.trim())}
                className="liquid-glass-primary flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12px] font-semibold disabled:cursor-not-allowed disabled:opacity-55"
              >
                {isSavingDiscogs ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Check className="h-3.5 w-3.5" />
                )}
                <span>Enregistrer</span>
              </button>

              {(hasDiscogsToken || discogsToken.trim().length > 0) && (
                <button
                  type="button"
                  disabled={isLoadingKeys || isSavingDiscogs}
                  onClick={() => void persistDiscogsToken("")}
                  className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-2 text-[12px] font-medium text-secondary hover:text-rose-400 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  <span>Effacer</span>
                </button>
              )}
            </div>

            <div className="pt-0.5">
              <button
                type="button"
                onClick={() => setShowDiscogsGuide((prev) => !prev)}
                aria-expanded={showDiscogsGuide}
                className="text-semantic-info inline-flex cursor-pointer items-center gap-1.5 text-[11.5px] font-medium hover:underline"
              >
                <HelpCircle className="h-3.5 w-3.5" />
                <span>Comment obtenir un token Discogs ?</span>
                <ChevronDown
                  className={`h-3.5 w-3.5 transition-transform ${
                    showDiscogsGuide ? "rotate-180" : ""
                  }`}
                />
              </button>

              {showDiscogsGuide && (
                <div className="mt-2 space-y-1.5 rounded-[12px] border border-[var(--content-border)] bg-[var(--content-bg)] px-3.5 py-2.5 text-[11.5px] leading-relaxed text-secondary">
                  <p>
                    Utile pour identifier vinyles, maxis club et pressages
                    physiques (stocké localement dans{" "}
                    <code className="break-all text-primary">{envFilePath}</code>
                    , exclu de Git).
                  </p>
                  <ol className="list-inside list-decimal space-y-1">
                    <li>
                      Ouvrir{" "}
                      <code className="text-primary">
                        discogs.com/settings/developers
                      </code>{" "}
                      et se connecter.
                    </li>
                    <li>
                      Cliquer sur{" "}
                      <strong className="text-primary">
                        Generate new token
                      </strong>
                      .
                    </li>
                    <li>
                      Copier le <em>Personal Access Token</em> généré et le
                      coller ci-dessus.
                    </li>
                  </ol>
                </div>
              )}
            </div>
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
