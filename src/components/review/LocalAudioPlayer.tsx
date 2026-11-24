import { useEffect, useRef, useState } from "react";
import { Pause, Play, Volume2, VolumeX } from "lucide-react";
import { isTauriEnvironment, toLocalAssetUrl } from "../../services/tauriBridge";

interface LocalAudioPlayerProps {
  sourcePath: string;
  fileName: string;
}

function formatSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const whole = Math.floor(seconds);
  const mins = Math.floor(whole / 60);
  const secs = whole % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

export function LocalAudioPlayer({
  sourcePath,
  fileName,
}: LocalAudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(0.85);
  const [isMuted, setIsMuted] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  const isNative = isTauriEnvironment();
  const assetUrl = isNative ? toLocalAssetUrl(sourcePath) : "";

  useEffect(() => {
    const audio = audioRef.current;
    setIsPlaying(false);
    setCurrentTime(0);
    setDuration(0);
    setLoadError(null);
    if (audio) {
      audio.pause();
      audio.currentTime = 0;
      audio.volume = volume;
      audio.muted = isMuted;
      audio.load();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourcePath]);

  useEffect(() => {
    const audio = audioRef.current;
    return () => {
      if (audio) {
        audio.pause();
        audio.currentTime = 0;
      }
    };
  }, []);

  const togglePlay = async () => {
    const audio = audioRef.current;
    if (!audio || !isNative) return;
    setLoadError(null);

    if (isPlaying) {
      audio.pause();
      setIsPlaying(false);
      return;
    }

    try {
      await audio.play();
      setIsPlaying(true);
    } catch {
      setIsPlaying(false);
      setLoadError(
        `Lecture impossible pour « ${fileName} » dans la WebView.`,
      );
    }
  };

  const handleSeek = (nextTime: number) => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.currentTime = nextTime;
    setCurrentTime(nextTime);
  };

  const toggleMute = () => {
    const audio = audioRef.current;
    const nextMuted = !isMuted;
    setIsMuted(nextMuted);
    if (audio) {
      audio.muted = nextMuted;
    }
  };

  const handleVolumeChange = (nextVol: number) => {
    const audio = audioRef.current;
    setVolume(nextVol);
    if (nextVol > 0 && isMuted) {
      setIsMuted(false);
      if (audio) audio.muted = false;
    }
    if (audio) {
      audio.volume = nextVol;
    }
  };

  return (
    <div className="rounded-[16px] border border-[var(--content-border)] bg-[var(--content-card-bg)] px-3.5 py-2.5">
      {isNative && (
        <audio
          ref={audioRef}
          src={assetUrl}
          preload="metadata"
          onLoadedMetadata={(e) =>
            setDuration(e.currentTarget.duration || 0)
          }
          onTimeUpdate={(e) => setCurrentTime(e.currentTarget.currentTime)}
          onEnded={() => {
            setIsPlaying(false);
            setCurrentTime(0);
          }}
          onError={() => {
            setIsPlaying(false);
            setLoadError(
              `Format ou accès audio non lisible pour « ${fileName} ».`,
            );
          }}
        />
      )}

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => void togglePlay()}
          disabled={!isNative}
          aria-label={isPlaying ? "Mettre en pause" : "Lire l'extrait local"}
          title={
            isNative
              ? isPlaying
                ? "Pause"
                : "Écouter le fichier local"
              : "Lecture audio disponible dans l'app desktop Tauri"
          }
          className="liquid-glass-pill flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full text-primary disabled:cursor-not-allowed disabled:opacity-45"
        >
          {isPlaying ? (
            <Pause className="h-4 w-4 text-sky-400" strokeWidth={2.2} />
          ) : (
            <Play className="ml-0.5 h-4 w-4 text-sky-400" strokeWidth={2.2} />
          )}
        </button>

        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          <span className="w-9 shrink-0 text-right text-[11px] font-medium tabular-nums text-secondary">
            {formatSeconds(currentTime)}
          </span>
          <input
            type="range"
            min={0}
            max={duration > 0 ? duration : 100}
            step={0.1}
            value={duration > 0 ? currentTime : 0}
            disabled={!isNative || duration <= 0}
            onChange={(e) => handleSeek(Number(e.target.value))}
            aria-label="Position de lecture"
            className="h-1.5 flex-1 cursor-pointer accent-sky-400 disabled:cursor-default disabled:opacity-45"
          />
          <span className="w-9 shrink-0 text-[11px] font-medium tabular-nums text-muted">
            {formatSeconds(duration)}
          </span>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <button
            type="button"
            onClick={toggleMute}
            disabled={!isNative}
            aria-label={isMuted ? "Réactiver le son" : "Couper le son"}
            className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-full text-secondary hover:bg-white/[0.06] hover:text-primary disabled:opacity-45"
          >
            {isMuted || volume === 0 ? (
              <VolumeX className="h-3.5 w-3.5" />
            ) : (
              <Volume2 className="h-3.5 w-3.5" />
            )}
          </button>
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={isMuted ? 0 : volume}
            disabled={!isNative}
            onChange={(e) => handleVolumeChange(Number(e.target.value))}
            aria-label="Volume"
            className="w-16 cursor-pointer accent-sky-400 disabled:opacity-45"
          />
        </div>
      </div>

      {loadError && (
        <p className="mt-1.5 text-[11px] text-amber-400">{loadError}</p>
      )}
    </div>
  );
}
