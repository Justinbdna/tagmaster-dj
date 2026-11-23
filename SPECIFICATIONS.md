# SPECIFICATIONS.md — Tagmaster DJ

## 1. Vision & Identité du Projet
* **Nom :** Tagmaster DJ
* **Type :** Application Desktop locale, 100 % Open Source (Licence MIT).
* **Objectif :** Nettoyer, standardiser, tagger automatiquement (métadonnées + pochettes HD + paroles), convertir et exporter des bibliothèques musicales locales (téléchargements YouTube/SoundCloud, achats, et plus tard rips de CD physiques) pour une compatibilité parfaite sur macOS/Windows, iPod, PSP 3000 (CFW) et serveur Homelab (Jellyfin/Navidrome).
* **Philosophie :** Local-first, aucune dépendance à un serveur distant propriétaire, résilience totale aux erreurs réseau, et contrôle humain final en cas de doute.

---

## 2. Stack Technique & Environnement
* **Framework Desktop :** Tauri 2.0 (faible empreinte RAM ~30-40 Mo, exécutables natifs `.exe` Windows et `.dmg` macOS).
* **Frontend :** React + TypeScript + Vite + Tailwind CSS.
* **Backend :** Rust (gestion système, appels API `reqwest`, lecture/écriture des tags audio via `lofty` ou `id3`, gestion du cache local).
* **Outils externes intégrés / pilotés par le backend :**
  * `FFmpeg` : Conversion audio (MP3 320 kbps) et redimensionnement/compression des pochettes.
  * `yt-dlp` : Extraction de métadonnées YouTube et SoundCloud sans clé API.
* **Développement :** Initialisé sur Windows 11 (`C:\Users\<User>\Documents\tagmaster-dj`), synchronisé via GitHub pour reprise et compilation ultérieure sur macOS (Apple Silicon).

---

## 3. Charte Graphique & UI/UX
### Thème Principal (MVP — Obligatoire)
* **Style :** Glassmorphism moderne inspiré d'iOS 26 / iOS 27 (effet verre liquide / translucide, flous d'arrière-plan subtils `backdrop-blur`, bordures fines semi-transparentes, typographie claire, contrastes propres, mode sombre par défaut avec option clair).
* **Ergonomie :** Interface épurée, non surchargée, pensée pour le glisser-déposer rapide et la lisibilité de l'avancement.

### Thème Secondaire (V2 — Optionnel / Easter Egg)
* **Style :** Skin rétro **Y2K / Winamp** activable dans les paramètres (non prioritaire pour le MVP).

---

## 4. Pipeline Audio & Règles de Métadonnées
### 4.1. Nettoyage ("Table Rase")
* Avant d'écrire les nouveaux tags, **supprimer intégralement** les anciens tags sales/publicitaires (ex: commentaires `www.mediahuman.com`, `free-mp3`, encodeurs étranges).

### 4.2. Cascade de Sources (API & Scraping)
* **Région forcée :** Base de données anglophone (`US` / `UK`) pour uniformiser les genres (Pop, R&B, Hip-Hop, Electronic) et éviter les catégories éclatées.
* **Anti-ban (Rate Limiting) :** Pause automatique (`sleep` de 1 à 2 secondes) entre chaque requête API.
* **Ordre de priorité :**
  1. **Apple Music (iTunes Search API)** : Source principale pour les sorties officielles (Titre, Artiste, Album, Numéro de piste, Numéro de disque, Année, Genre, Pochette HD). Aucune clé requise.
  2. **YouTube & SoundCloud (via `yt-dlp`)** : Source prioritaire pour les remixes, mashups, versions *Live*, *Sped Up*, *Chopped*, *Acoustic*, *Instrumental*.
  3. **Amazon Music / Google (fallback pochette)** : Recherche complémentaire si aucune pochette n'est trouvée.
  4. **Genius API** : Récupération des paroles (si clé présente dans `.env`, sinon ignoré sans erreur).
  5. **Discogs API (V2 / CD)** : Identification précise des éditions physiques CD/Vinyles.

### 4.3. Règles Strictes de Tagging
* **Featurings (Anti-éclatement d'album) :**
  * Les featurings sont inscrits dans le champ **Titre** : `Chulo pt.2 (feat. Tokischa & Young Miko)`.
  * Le champ **Artiste de l'album (`Album Artist`)** est verrouillé sur l'artiste principal unique (ex: `Bad Gyal`) pour toutes les pistes de l'album.
* **Singles :** Si le morceau est un single, le champ `Album` reprend le nom du single.
* **Compilations / BO :** Si détecté comme compilation, `Album Artist` = `Various Artists`, tout en conservant le vrai interprète dans `Artiste`.
* **Albums Multi-Disques (Deluxe / Rééditions) :** Remplissage obligatoire du tag `Disc Number` (ex: `1/2`, `2/2`) pour éviter le mélange des pistes.
* **Mots-clés intouchables :** Si le fichier source contient `Live`, `Acoustic`, `Sped Up`, `Chopped`, `Instrumental`, `Remix`, `Mashup` → ne jamais laisser Apple Music écraser avec la version studio ; basculer sur `yt-dlp` ou envoyer en file de révision manuelle.
* **Sanitizer (Caractères spéciaux) :** Pour les profils rétro (iPod / PSP), nettoyer les emojis et caractères non supportés. Si le titre devient illisible ou ambigu, envoyer en file de révision manuelle.

---

## 5. Exports, Formats & Profils Matériels
### 5.1. Sécurité des Fichiers
* **Ne jamais écraser les fichiers originaux.** Tous les fichiers traités sont copiés/exportés dans un dossier de sortie dédié (`Documents/Tagmaster_DJ_Export`).

### 5.2. Mode d'Arborescence (Toggle dans l'UI)
* **Mode Dynamique :** `Export / Artiste / Album / Artiste - Titre.mp3`
* **Mode Plat :** `Export / Artiste - Titre.mp3` (tout dans un seul dossier pour drag & drop rapide).
* **Sécurité Windows (Limite 256 caractères) :** Si le chemin complet approche la limite Windows, tronquer intelligemment le nom du fichier physique (`...`) tout en conservant le titre complet dans les métadonnées internes.

### 5.3. Formats & Profils d'Export (via FFmpeg)
* **Mode Passthrough (Tel quel) :** Conserve le format d'origine (`.mp3`, `.m4a`, `.aac`, `.flac`) et met uniquement à jour les tags.
* **Mode Force MP3 :** Convertit tout (`.wav`, `.flac`, `.m4a`, `.aac`) en `.mp3` 320 kbps.
* **Profils dédiés :**
  * **Standard / iPhone / Mac :** Pochette HD (jusqu'à 1000x1000px).
  * **Profil iPod :** Compression de la pochette (600x600px max) + Sanitizer activé.
  * **Profil PSP 3000 (CFW) :** Conversion forcée en MP3 320 kbps + pochette compressée (400x400px max) + Sanitizer activé.

---

## 6. Résilience, Mémoire & Expérience Utilisateur
* **Barre de progression en temps réel :** Affichage du fichier en cours, du compteur (`X / Y`) et du pourcentage.
* **Persistance d'état (`cache.json`) :** Sauvegarde locale après chaque fichier traité avec succès.
  * En cas de coupure réseau ou crash API : Pop-up avec 3 choix : **[Réessayer la connexion]** / **[Exporter les X pistes déjà prêtes]** / **[Quitter]**.
  * À la réouverture : Affichage d'un check vert sur les pistes déjà traitées + proposition de reprendre les fichiers restants.
* **File d'attente de révision (Post-Processing Queue) :** Ne jamais bloquer un traitement par lots sur un fichier problématique (titre cryptique, pochette introuvable, conflit de version). Passer à la suite et regrouper tous les cas douteux dans un onglet final pour édition manuelle (titre, artiste, import manuel d'image).
* **Lecteur Audio Intégré & Comparateur A/B (façon WhoSampled) :**
  * Écoute directe des fichiers locaux dans l'interface pour vérifier un titre cryptique.
  * En cas de **doublon détecté**, affichage côte à côte (Piste A à gauche / Piste B à droite) avec lecture audio pour comparer la qualité/version et choisir : Garder A, Garder B, Garder les deux, ou Supprimer les deux.
* **Paramètres & Sécurité :** Onglet dédié pour saisir les clés API (Genius, Discogs), stockées exclusivement dans un fichier `.env` local (jamais commité).

---

## 7. Roadmap & Workflow Git
* **Branches :**
  * `main` : Versions stables uniquement.
  * `staging` : Intégration et tests avant mise en production.
  * `feat/<nom-feature>` : Une branche par nouvelle fonctionnalité.
* **Commits conventionnels :** `feat:`, `fix:`, `docs:`, `style:`, `refactor:`.
* **Sprints :**
  * **Sprint 1 (MVP) :** Setup Tauri 2.0 + React + Glassmorphism iOS 26/27, sélection de dossier, appel API Apple Music US + nettoyage/écriture tags ID3, export Plat/Dynamique sécurisé, barre de progression.
  * **Sprint 2 (Résilience & Outils) :** `yt-dlp` (YouTube/SoundCloud), Genius (paroles), `cache.json` (reprise après crash), File de révision manuelle, Lecteur audio + Comparateur A/B doublons, FFmpeg (Profils iPod/PSP).
  * **Sprint 3 (Hardware & Bonus) :** Module CD Ripping (lecteur optique + API Discogs), Skin Y2K/Winamp, GitHub Actions (build auto `.exe` + `.dmg`).