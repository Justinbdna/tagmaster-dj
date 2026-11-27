export function formatFileSize(bytes: number): string {
  if (bytes <= 0) return "0 o";
  const units = ["o", "Ko", "Mo", "Go"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** exponent;
  const formatted =
    exponent === 0
      ? Math.round(value).toString()
      : value.toLocaleString("fr-FR", {
          minimumFractionDigits: value < 10 ? 1 : 0,
          maximumFractionDigits: 1,
        });
  return `${formatted} ${units[exponent]}`;
}

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  const total = Math.round(seconds);
  const hrs = Math.floor(total / 3600);
  const mins = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hrs > 0) {
    return `${hrs}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }
  return `${mins}:${String(secs).padStart(2, "0")}`;
}

export function formatDuplicateBadgeLabel(count: number): string {
  if (count <= 2) return "Doublon";
  if (count === 3) return "Triplé";
  if (count === 4) return "Quadruplé";
  return `×${count} copies`;
}
