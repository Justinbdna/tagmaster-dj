import type { HardwareProfile } from "../../types/track";

interface IconProps {
  className?: string;
}

/** Silhouette minimaliste d'iPhone (profil Standard) */
export function IphoneIcon({ className = "h-4 w-4" }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.65}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <rect x="6.5" y="2.25" width="11" height="19.5" rx="2.6" />
      <line x1="10.2" y1="5.2" x2="13.8" y2="5.2" />
      <line x1="10" y1="19" x2="14" y2="19" />
    </svg>
  );
}

/** Silhouette minimaliste d'iPod Classic avec molette cliquable iconique */
export function IpodClassicIcon({ className = "h-4 w-4" }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.65}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <rect x="5.5" y="2.25" width="13" height="19.5" rx="2.4" />
      <rect x="7.8" y="4.6" width="8.4" height="5.5" rx="0.9" />
      <circle cx="12" cy="15.7" r="3.4" />
      <circle cx="12" cy="15.7" r="1.1" />
    </svg>
  );
}

/** Silhouette horizontale minimaliste de PlayStation Portable (PSP) */
export function PspIcon({ className = "h-4 w-4" }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.65}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <rect x="1.75" y="6.25" width="20.5" height="11.5" rx="5.25" />
      <rect x="7.2" y="8.6" width="9.6" height="6.8" rx="1" />
      {/* Croix directionnelle gauche */}
      <line x1="4.7" y1="10.7" x2="4.7" y2="13.3" />
      <line x1="3.4" y1="12" x2="6" y2="12" />
      {/* Boutons d'action droits */}
      <circle cx="19.3" cy="10.6" r="0.55" fill="currentColor" stroke="none" />
      <circle cx="19.3" cy="13.4" r="0.55" fill="currentColor" stroke="none" />
      <circle cx="17.9" cy="12" r="0.55" fill="currentColor" stroke="none" />
      <circle cx="20.7" cy="12" r="0.55" fill="currentColor" stroke="none" />
    </svg>
  );
}

export const HARDWARE_PROFILE_LABELS: Record<HardwareProfile, string> = {
  standard: "Standard",
  ipod: "iPod",
  psp: "PSP",
};

export function HardwareProfileIcon({
  profile,
  className = "h-4 w-4",
}: {
  profile: HardwareProfile;
  className?: string;
}) {
  if (profile === "ipod") return <IpodClassicIcon className={className} />;
  if (profile === "psp") return <PspIcon className={className} />;
  return <IphoneIcon className={className} />;
}
