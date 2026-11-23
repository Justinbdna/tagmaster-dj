# Tagmaster DJ

Application desktop **100 % open-source (Licence MIT)**, locale (*local-first*), conçue pour nettoyer, standardiser, tagger automatiquement (métadonnées, pochettes HD, paroles), convertir et exporter vos bibliothèques musicales locales avec une compatibilité optimale sur **Windows / macOS, iPod, PSP 3000 (CFW)** et serveurs homelab (**Jellyfin / Navidrome**).

---

## Stack Technique

- **Framework Desktop :** [Tauri 2.0](https://v2.tauri.app/) (binaire natif léger `.exe` / `.dmg`)
- **Frontend :** React 19 + TypeScript + Vite + Tailwind CSS (UI Glassmorphism inspirée d'iOS 26/27)
- **Backend :** Rust (appels API, règles métier de tagging, export sécurisé non destructif, persistance `cache.json`)
- **Outils externes pilotés :** `FFmpeg` (conversion MP3 320 kbps & compression pochettes) et `yt-dlp` (remixes, lives, versions SoundCloud/YouTube)

Pour le cahier des charges complet, consultez [`SPECIFICATIONS.md`](./SPECIFICATIONS.md).

---

## Prérequis de Développement (Windows 11)

- **Node.js** (v20+) & **npm**
- **Rust** (`rustc` + `cargo` via [rustup](https://rustup.rs/))
- **Microsoft C++ Build Tools** (Visual Studio Build Tools avec *Desktop development with C++* + Windows 10/11 SDK)
- **WebView2 Runtime** (préinstallé sur Windows 11)
- **FFmpeg** & **yt-dlp** (requis dès le Sprint 2 pour la conversion, les profils iPod/PSP et l'extraction YouTube/SoundCloud)

---

## Installation & Lancement

```bash
# 1. Installer les dépendances frontend
npm install

# 2. Configurer les clés API optionnelles (Genius, Discogs)
cp .env.example .env

# 3. Lancer l'application desktop en mode développement
npm run tauri dev

# 4. Compiler le build de production
npm run tauri build
```

> **Sécurité :** Le fichier `.env` et le fichier d'état local `cache.json` sont strictement exclus de Git. Les fichiers audio originaux ne sont **jamais** écrasés : tous les exports sont écrits dans `Documents/Tagmaster_DJ_Export`.

---

## Architecture du Projet

```text
tagmaster-dj/
├── SPECIFICATIONS.md        # Cahier des charges complet
├── .env.example             # Modèle des clés API optionnelles
├── src/                     # Frontend React + TypeScript + Tailwind CSS
│   ├── components/          # Composants UI Glassmorphism modulaires
│   ├── hooks/               # Hooks React métier et état applicatif
│   ├── pages/               # Vues principales (Workspace, File de révision, Paramètres)
│   ├── services/            # Pont IPC typé vers le backend Tauri
│   ├── styles/              # Styles globaux & tokens Tailwind CSS
│   └── types/               # Interfaces TypeScript partagées
└── src-tauri/               # Backend Rust Tauri 2.0
    └── src/
        ├── api/             # Cascade de sources (Apple Music US, yt-dlp, Genius, Discogs)
        ├── tagger/          # Nettoyage Table Rase, règles featurings/disques & sanitizer
        ├── exporter/        # Export Plat/Dynamique, garde 256 car. Windows & profils
        ├── cache/           # Persistance locale cache.json & reprise après coupure
        ├── models.rs        # Structures de données sérialisées IPC
        └── lib.rs           # Point d'entrée Tauri & commandes exposées
```

---

## Workflow Git

- `main` : versions stables uniquement
- `staging` : intégration et validation pré-production
- `feat/<nom-feature>` : développement isolé par fonctionnalité
- Commits conventionnels : `feat:`, `fix:`, `docs:`, `style:`, `refactor:`

---

## Licence

Distribué sous licence **MIT**. Voir [`LICENSE`](./LICENSE) pour plus d'informations.
