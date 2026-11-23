import { useEffect, useState } from "react";
import { checkBackendHealth } from "../services/tauriBridge";
import type { HealthStatus } from "../types/track";

interface AppHealthState {
  health: HealthStatus | null;
  isTauriRuntime: boolean;
}

export function useAppHealth(): AppHealthState {
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [isTauriRuntime, setIsTauriRuntime] = useState(false);

  useEffect(() => {
    let active = true;

    checkBackendHealth()
      .then((result) => {
        if (active) {
          setHealth(result);
          setIsTauriRuntime(true);
        }
      })
      .catch(() => {
        if (active) {
          setIsTauriRuntime(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  return { health, isTauriRuntime };
}
