import { invoke } from "@tauri-apps/api/core";
import type { HealthStatus } from "../types/track";

/**
 * Pont IPC typé entre le frontend React et le backend Rust Tauri 2.0.
 * Les commandes métier du Sprint 1 (scan de dossier, Apple Music US, tagging, export)
 * seront enregistrées ici au fil de l'implémentation.
 */
export async function checkBackendHealth(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
}
