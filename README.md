# Tagmaster DJ

Application desktop **100 % open-source (Licence MIT)**, locale (*local-first*), conçue pour nettoyer, standardiser, tagger automatiquement (métadonnées, pochettes HD, paroles synchronisées), dédoublonner, convertir et exporter vos bibliothèques musicales locales avec une compatibilité optimale sur **macOS / Windows, iPod, PSP** et serveurs homelab (**Jellyfin / Navidrome**).

---

## Fonctionnalités (`v0.3.0`)

- **Interface Liquid Glass native (Clair & Sombre)** : translucidité macOS native (`Translucide` / `Teinté`), modes focus pleine largeur, pastilles coulissantes et contrastes WCAG AA+.
- **Performances & gros lots (3 000+ pistes)** :
  - Identification en parallèle (pool borné à 3 workers, clients HTTP réutilisés) et export multi-threadé (`clamp(threads / 2, 2, 4)`).
  - Affichage immédiat des pochettes locales dès l'import (extraction asynchrone par lots + purge automatique du cache temporaire `tagmaster-dj-thumbs`).
  - Diagnostic intégré des dépendances **`ffmpeg`** et **`yt-dlp`** au démarrage.
- **Cascade multi-sources intelligente (`Auto`) & BPM** :
  - **Apple Music** (`US → GB → FR` + lookup catalogue artiste avec cache mémoire `artistId`)
  - **Deezer** (API publique sans clé, pochettes `1000×1000` + champ **BPM**)
  - **MusicBrainz + Cover Art Archive** (API publique, rate-limit 1 req/s)
  - **Discogs** (optionnel via `DISCOGS_TOKEN` dans `.env` pour vinyles & maxis club)
  - **YouTube & SoundCloud (`yt-dlp`)** : routage prioritaire automatique sur les versions protégées (`Live`, `Remix`, `Mashup`, `Sped Up`, `Slowed`, `Reverb`, `Nightcore`, `Chopped`, `Acoustic`, `Instrumental`) et recherche séparée à la volée.
  - Écriture standard du **BPM** (`TBPM` en ID3v2.3/2.4, atome entier signé `tmpo` sur MP4/M4A, `BPM` sur FLAC/OGG) + édition manuelle dans la File de révision.
- **File de révision & éditeur libre** :
  - Édition de n'importe quelle piste (même `Identifié` ou `Exporté`), lecteur audio local intégré et **Recherche rapide** avec sélecteur unifié à 7 sources (`Auto`, `Apple Music`, `Deezer`, `YouTube`, `SoundCloud`, `MusicBrainz`, `Discogs`).
  - **Sélecteur de 16 pochettes HD en ligne** + import local + **URL d'image directe** (JPG, PNG, WebP).
- **Paroles synchronisées temps réel** : récupération automatique via **LRCLIB** (sans clé) et fallback **Genius** (optionnel via `GENIUS_API_KEY` dans `.env`, ignoré sur les pistes `Instrumental`) avec écriture ID3v2 **`SYLT` + `USLT` + fichier compagnon `.lrc`**.
- **Persistance & résilience réseau** : sauvegarde atomique sérialisée dans `cache.json`, reprise de session au redémarrage et pop-up à 3 choix en cas de coupure réseau / rate-limit (**Réessayer**, **Exporter les pistes prêtes**, **Quitter (sauvegardé)**).
- **Détection de doublons & Comparateur audio A/B** : repérage automatique des copies (`__2`, `_1`, `(1)` et `Artiste + Titre` normalisés), comparaison côte à côte avec arrêt mutuel des lecteurs audio et arbitrage en 1 clic (**Garder A**, **Garder B**, **Garder les deux**, **Supprimer les deux**).
- **Moteur `FFmpeg`, dossier d'export personnalisable & profils matériels** (strictement non destructifs, `Documents/Tagmaster_DJ_Export` par défaut ou dossier personnalisé, badge **`Exporté`** cliquable vers le Finder / Explorateur) :
  - **`Standard`** : audio `Original` (ou `MP3 320k`), pochette HD max `1000×1000`, `SYLT` + `USLT` + `.lrc`, UTF-8.
  - **`iPod`** : Passthrough MP3/M4A (conversion MP3 320k si FLAC/WAV), pochette JPEG baseline max `600×600`, Sanitizer FAT32, ID3v2.3.
  - **`PSP`** : conversion forcée MP3 CBR `320 kbps` (`44,1 kHz`, stéréo), pochette JPEG baseline max `400×400`, Sanitizer ASCII strict (Memory Stick Pro Duo / XMB), ID3v2.3 sans `.lrc`.
- **Packaging natif & CI/CD** : installeurs macOS (`.dmg`) et Windows (`.exe` / `.msi`) construits automatiquement via GitHub Actions à chaque release.

---

## Stack Technique

- **Framework Desktop :** [Tauri 2.0](https://v2.tauri.app/) (binaire natif `.dmg` / `.exe`)
- **Frontend :** React 19 + TypeScript + Vite + Tailwind CSS v4
- **Backend :** Rust (`lofty`, `reqwest`, `tokio`, `walkdir`, `regex`, `serde`)
- **Outils externes pilotés :** `FFmpeg` (transcodage MP3 320k & pochettes JPEG baseline) et `yt-dlp` (métadonnées YouTube & SoundCloud)

Consultez [`CHANGELOG.md`](./CHANGELOG.md) pour l'historique des versions et [`SPECIFICATIONS.md`](./SPECIFICATIONS.md) pour le cahier des charges.

---

## Prérequis de Développement

- **Node.js** (v20+) & **npm**
- **Rust** (`rustc` + `cargo` via [rustup](https://rustup.rs/))
- **FFmpeg** & **yt-dlp** dans le `PATH` (ex. `brew install ffmpeg yt-dlp` sur macOS)
- **Windows 11 :** Microsoft C++ Build Tools (*Desktop development with C++*) + WebView2 Runtime

---

## Installation & Lancement

```bash
# 1. Installer les dépendances frontend
npm install

# 2. (Optionnel) Configurer les clés API locales (Genius, Discogs)
cp .env.example .env

# 3. Lancer l'application desktop en mode développement
npm run tauri dev

# 4. Compiler le build de production
npm run tauri build
```

> **Sécurité :** `.env` et `cache.json` sont strictement exclus de Git. Les fichiers audio originaux ne sont **jamais** modifiés : tous les exports sont écrits dans `Documents/Tagmaster_DJ_Export`.

---

## Architecture du Projet

```text
tagmaster-dj/
├── CHANGELOG.md             # Historique des releases
├── SPECIFICATIONS.md        # Cahier des charges complet
├── .env.example             # Modèle des clés API optionnelles (Genius, Discogs)
├── src/                     # Frontend React 19 + TypeScript + Tailwind CSS v4
│   ├── components/          # UI Liquid Glass (Bibliothèque, Révision, Doublons A/B, Modales, Paramètres)
│   ├── hooks/               # État applicatif, persistance session, thème
│   ├── pages/               # WorkspacePage
│   ├── services/            # Pont IPC typé vers le backend Tauri
│   ├── styles/              # Tokens Liquid Glass & contraste WCAG AA+ Clair/Sombre
│   └── types/               # Interfaces TypeScript partagées
└── src-tauri/               # Backend Rust Tauri 2.0
    └── src/
        ├── api/             # Apple Music, Deezer, MusicBrainz, Discogs, yt-dlp, LRCLIB/Genius
        ├── cache/           # Persistance atomique cache.json & reprise de session
        ├── duplicates.rs    # Détection de doublons & classement par qualité
        ├── exporter/        # Export Plat/Dynamique, garde 256 car. Windows & profils
        ├── ffmpeg/          # Transcodage MP3 320k & pochettes JPEG baseline (SOF0)
        ├── scanner/         # Scan récursif audio & lecture propriétés techniques
        ├── tagger/          # Table Rase, règles featurings/disques & sanitizer FAT32/ASCII
        ├── models.rs        # Structures de données IPC
        └── lib.rs           # Point d'entrée Tauri & commandes exposées
```

---

## Licence

Distribué sous licence **MIT**. Voir [`LICENSE`](./LICENSE) pour plus d'informations.
