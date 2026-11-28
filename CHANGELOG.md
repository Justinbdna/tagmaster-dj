# Changelog

Toutes les évolutions notables de **Tagmaster DJ** sont documentées dans ce fichier.

---

## [0.3.0] — 2026-11-28

### Ajouté
- **Packaging natif macOS / Windows & CI/CD GitHub Actions** :
  - Configuration Tauri 2.0 complète pour la génération d'installeurs natifs **macOS (`.dmg`, `.app`)** et **Windows (`.exe` NSIS, `.msi`)**.
  - Workflows `.github/workflows/ci.yml` (validation Rust macOS/Ubuntu + TypeScript/Vite sur chaque PR) et `.github/workflows/release.yml` (publication automatique des builds multi-plateformes sur chaque tag `v*`).
  - Diagnostic intégré au démarrage des dépendances externes **`ffmpeg`** et **`yt-dlp`** (bannière discrète avec commande d'installation adaptée à l'OS : `brew` / `winget` / `apt`).
- **Parallélisme maîtrisé & performances gros lots (3 000+ pistes)** :
  - Identification en parallèle via pool asynchrone borné à **3 workers simultanés** (clients HTTP réutilisés via `OnceLock`, rate-limiters indépendants par source, modale de résilience réseau intacte).
  - Export parallèle (FFmpeg + Table Rase) adapté aux cœurs logiques (`clamp(threads / 2, 2, 4)`), écritures `cache.json` sérialisées et regroupées hors thread principal, lignes de file mémoïsées (`React.memo`).
- **Pochettes locales dès l'import & purge disque automatique** :
  - Extraction asynchrone en arrière-plan des pochettes embarquées dans `tagmaster-dj-thumbs` (affichage immédiat cadré en `object-cover`, remplacé automatiquement par la pochette HD après identification).
  - Nettoyage automatique des miniatures orphelines au démarrage et purge immédiate sur **Vider**, **Nouvelle session** ou suppression de piste.
- **Révélation des exports dans le Finder / Explorateur** :
  - Badge **`Exporté`** cliquable dans la bibliothèque (`open -R` sur macOS, `explorer /select,` sur Windows, `xdg-open` sur Linux) et bouton compact **`Voir`** dans le toast de fin d'export.
- **Dossier d'export personnalisable** :
  - Sélection d'un dossier cible personnalisé (disque externe, clé USB, `MUSIC/` PSP…) dans **Paramètres → Type d'export** avec bouton **Par défaut**, validation préalable en écriture et raccourci dynamique dans la **Sidebar**.
- **Support du tag BPM (`TBPM` / `tmpo` / `BPM`)** :
  - Extraction sans surcoût réseau depuis l'API Deezer et conservation du BPM existant des fichiers sources.
  - Écriture standard **`TBPM`** (ID3v2.3/2.4 sur MP3), atome entier signé **`tmpo`** (`DataType::BeSignedInteger` sur MP4/M4A) et **`BPM`** (Vorbis/FLAC), plus champ discret **BPM** dans le formulaire d'édition de la File de révision.

### Qualité & Sécurité
- **66 tests unitaires Rust** couvrant également la concurrence bornée, la purge sécurisée du cache de miniatures, la validation du dossier d'export, `reveal_in_file_manager` (caractères spéciaux) et l'écriture/relecture BPM sur MP3 et M4A.
- `cargo clippy --all-targets -- -D warnings`, `cargo test`, `tsc --noEmit` et `npm run build` validés à **0 erreur, 0 warning**.

---

## [0.2.0] — 2026-11-27

### Ajouté
- **File de révision interactive & éditeur libre** :
  - Édition de n'importe quelle piste de la bibliothèque (y compris `Identifié` et `Exporté`), lecteur audio local intégré et mode focus pleine largeur sans sidebar.
  - Sélecteur de **16 pochettes HD en ligne** (Apple Music), import d'image locale et saisie d'**URL d'image directe** (JPG, PNG, WebP) avec validation.
- **Support multi-sources & sélecteur unifié à 7 options** :
  - Nouveaux clients backend **Deezer** (API publique, `cover_xl` `1000×1000`), **MusicBrainz + Cover Art Archive** (rate-limit 1 req/s) et **Discogs** (optionnel via `DISCOGS_TOKEN` dans `.env`).
  - Fallback **`yt-dlp`** (YouTube & SoundCloud) prioritaire sur les versions protégées (`Live`, `Remix`, `Mashup`, `Sped Up`, `Slowed`, `Reverb`, `Nightcore`, `Chopped`, `Acoustic`, `Instrumental`) avec séparation explicite `YouTube` / `SoundCloud` en recherche rapide.
  - Cascade `Auto` intelligente : Apple Music (`US → GB → FR`) → Deezer → MusicBrainz → Discogs → YouTube / SoundCloud.
  - Cache mémoire `(storefront, artiste)` sur les lookups `artistId` Apple Music.
- **Paroles synchronisées temps réel & Genius API** :
  - Récupération automatique des paroles synchronisées **LRCLIB** (sans clé) + fallback **Genius API** (`GENIUS_API_KEY` dans `.env`, ignoré sur `Instrumental`).
  - Écriture ID3v2 **`SYLT` + `USLT`** (en conservant la pochette `APIC`) et génération du fichier compagnon **`.lrc`**.
- **Persistance `cache.json` & résilience réseau** :
  - Sauvegarde atomique de l'état de la file et reprise de session au redémarrage (**Reprendre la session précédente** / **Nouvelle session**).
  - Interception automatique des coupures réseau et erreurs HTTP `429`/`5xx` avec modale à 3 choix (**Réessayer**, **Exporter les pistes prêtes**, **Quitter (sauvegardé)**).
- **Détection de doublons & Comparateur audio A/B** :
  - Détection automatique par couple `Artiste + Titre` normalisé et variantes de copies (`__2`, `_1`, `(1)`), badges `Doublon` / `Triplé` / `Quadruplé`.
  - Vue focus A/B côte à côte avec arrêt mutuel automatique des lecteurs audio, comparaison technique (format, bitrate/fréquence, taille, dossier) et 4 actions (**Garder A**, **Garder B**, **Garder les deux**, **Supprimer les deux**).
- **Moteur `FFmpeg` & profils d'export matériels** :
  - Modes audio **`Original`** (Passthrough) et **`MP3 320k`** (`libmp3lame`, CBR 320 kbps, `44,1 kHz`, stéréo).
  - Redimensionnement carré des pochettes en **JPEG baseline** (`SOF0`, anti-progressif / anti-WebP).
  - 3 profils matériels avec icônes SVG sur mesure : **`Standard`** (`1000×1000`, `SYLT`/`.lrc`, UTF-8), **`iPod`** (`600×600` JPEG baseline, FAT32, ID3v2.3) et **`PSP`** (`MP3 320k`, `400×400` JPEG baseline, ASCII strict, ID3v2.3 sans `.lrc`).
- **Interface Liquid Glass macOS & finitions UX** :
  - Transparence macOS native (`Translucide` / `Teinté`) en Mode Clair et Mode Sombre, contrastes WCAG AA+, pastilles coulissantes, menu déroulant compact des formats et popover de profil rapide dans la barre d'export.

### Qualité & Sécurité
- **57 tests unitaires Rust** couvrant toutes les sources, règles métier, sanitizer FAT32/ASCII, `FFmpeg` JPEG baseline, `cache.json` et détection de doublons.
- `cargo clippy --all-targets`, `cargo check`, `tsc --noEmit` et `npm run build` validés à **0 erreur, 0 warning**.
- `.env` et `cache.json` strictement exclus de Git.

---

## [0.1.0] — Sprint 1 (MVP)

### Ajouté
- Socle Tauri 2.0 + React 19 + TypeScript + Tailwind CSS v4 + Rust.
- Scanner récursif audio (`MP3`, `M4A`, `AAC`, `FLAC`, `WAV`) avec détection des mots-clés protégés.
- Identification Apple Music (`US → GB → FR`), règles de tagging strictes (featurings, `Album Artist`, compilations, multi-disques) et nettoyage Table Rase (`lofty`).
- Export non destructif Plat / Dynamique dans `Documents/Tagmaster_DJ_Export` avec garde de longueur Windows (248 car.).
