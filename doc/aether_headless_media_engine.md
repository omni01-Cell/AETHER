# AETHER — Le Couteau Suisse Headless de la Création Multimédia pour Agents IA

## TL;DR

**AETHER** est un moteur de création multimédia headless écrit en **Rust**, conçu spécifiquement pour être manipulé par des **agents IA** via un **DSL (Domain Specific Language)** en ligne de commande. Il fusionne cinq capacités fondamentales — **montage vidéo, animation, illustration, traitement audio et génération IA** — dans une architecture unifiée CLI/Daemon qui persiste l'état entre les commandes. Inspiré des patterns de **Vercel agent-browser** (snapshot+refs, 93% de réduction de contexte) et des architectures de **Natron/After Effects** (node-based compositing) et **Premiere Pro** (timeline éditorial), AETHER permet à un agent de réaliser un montage complet étape par étape : importer des médias, les découper, ajouter des transitions, générer des assets par IA, mixer l'audio, et exporter — le tout via un langage déclaratif conçu pour être compris et généré par des LLM. Le projet s'appuie sur **FFmpeg/libav*** pour le vidéo, **Symphonia/rubato/hound** pour l'audio, **image-rs/resvg/tiny-skia** pour les images, **lottie-rs** pour l'animation, et des **plugins asynchrones** pour les APIs de génération IA (Google Veo 3.1, Nano Banana 2, Wan 2.5, MiniMax Speech 2.5).

---

## 1. Vision : Un "Creative Engine" pour l'Ère des Agents IA

### 1.1 Le Problème : L'Asymétrie Créative entre IA et Outils

Les modèles d'IA générative ont aujourd'hui la capacité de concevoir des scripts vidéo complets, de rédiger des voix-off, d'imaginer des storyboards détaillés et de décrire des univers visuels avec une précision remarquable. Pourtant, un fossé structurel demeure : une fois le concept créatif élaboré, l'agent IA se retrouve démuni face à l'étape de production. Il ne peut ni lancer Adobe Premiere Pro, ni manipuler After Effects, ni même exécuter Audacity — car ces outils sont fondamentalement conçus pour des humains interagissant avec une interface graphique. Les solutions existantes comme **VibeStudio** (bash+FFmpeg via MCP) ou **auto-editor** (Python+FFmpeg) offrent des approches fragmentées qui se limitent à l'assemblage mécanique de clips, sans véritable modèle de composition, sans persistence d'état, et sans capacité de génération IA intégrée  [(Github)](https://github.com/wizenheimer/vibestudio) . Le résultat est un pipeline brisé où l'agent doit jongler entre une demi-douzaine d'outils hétéroclites, chacun avec sa propre syntaxe, ses propres limites et son absence de traçabilité.

Ce qui manque fondamentalement, c'est un **créatif middleware** : une couche d'abstraction unifiée qui traduit les intentions créatives d'un agent en opérations de production concrètes, persistantes et réversibles. Un outil qui reprend les concepts fondamentaux des logiciels professionnels — la timeline de Premiere Pro, le node-based compositing d'After Effects, le système de calques d'Illustrator, le pipeline audio d'Audacity, le rigging d'OpenToonz — mais entièrement dépourvu d'interface graphique, manipulable exclusivement via un langage déclaratif conçu pour être lu, écrit et raisonné par des modèles de langage.

### 1.2 La Solution : AETHER et son DSL Créatif

AETHER ("Advanced Engine for Theatrical and Electronic Rendering") est conçu comme ce middleware. Son architecture s'inscrit dans la lignée des outils headless qui ont émergé en 2025-2026 pour les agents IA, mais en repoussant considérablement les frontières. Où **agent-browser** de Vercel s'est concentré sur la navigation web (snapshot d'éléments interactifs, réfs `@e1`, `@e2`)  [(Github)](https://github.com/vercel-labs/agent-browser) , où **VibeStudio** s'est limité à l'enrobage FFmpeg en bash  [(Github)](https://github.com/wizenheimer/vibestudio) , AETHER ambitionne de couvrir l'intégralité du pipeline créatif multimédia.

Le pari central est linguistique : plutôt que de forcer l'agent à générer des commandes FFmpeg impératives ou des scripts Python opaques, AETHER expose un **DSL sémantique** qui parle le langage du montage. L'agent écrit `trim @v1 0:00:03 to 0:00:15` plutôt que de calculer des timestamps en frames et de construire une filtergraph FFmpeg. Il écrit `composite @i1 over @v2 --blend multiply --opacity 0.7` plutôt que de gérer des buffers de pixels. Le DSL est conçu pour être **prévisible, idempotent et inspectable** — trois qualités essentielles pour qu'un agent puisse raisonner sur l'état du projet et itérer de manière fiable.

### 1.3 Architecture Client-Daemon : Leçons de Vercel agent-browser

L'architecture de référence pour AETHER est directement inspirée de **agent-browser** de Vercel Labs, qui a démontré en 2026 la supériorité du pattern **CLI-Daemon** pour les workflows d'agents IA  [(Github)](https://github.com/vercel-labs/agent-browser) . Dans ce modèle, le **CLI Rust** (`aether-cli`) est un processus léger, stateless, qui parse les commandes et les transmet via **Unix Domain Socket** à un **daemon Rust** (`aether-daemon`) qui persiste en mémoire l'intégralité de l'état du projet. Le daemon gère les timelines, les graphs de composition, les assets, les jobs de rendu et les connexions aux APIs IA. Cette séparation offre trois avantages décisifs : **performance** (pas de coût de démarrage entre les commandes), **persistence** (l'état du montage survit à la commande), et **atomicité** (le CLI peut être relancé sans perdre le contexte créatif).

![Architecture AETHER](aether_architecture.png)

| Couche | Composant | Technologie | Responsabilité |
|--------|-----------|-------------|----------------|
| **① CLI** | `aether-cli` | Rust, clap, serde | Parse le DSL, valide, convertit en RPC |
| **② Daemon** | `aether-daemon` | Rust, tokio, UDS/TCP | État persistant, session, composition graph |
| **③ Core** | 4 Engines | Rust + libs natifs | Traitement vidéo, audio, image, animation |
| **④ IA** | Plugin System | reqwest, async | APIs Veo 3, Nano Banana, Wan, MiniMax |
| **⑤ Persistence** | SQLite + JSON | rusqlite, serde_json | Projet, timeline, composition, assets |
| **⑥ Output** | Encodeurs | FFmpeg, image-rs | MP4, MOV, WAV, PNG, SVG, ProRes, EXR |

---

## 2. Le DSL (Domain Specific Language) : Le Langage du Montage pour IA

### 2.1 Philosophie : Prévisible, Idempotent, Réversible

Le DSL d'AETHER est le cœur de l'expérience agent. Contrairement à un langage de programmation généraliste, il est **contextuel** : chaque commande s'exécute dans le cadre d'un projet ouvert, d'une timeline active, d'un graph de composition en cours d'édition. Cette approche réduit drastiquement la verbosité et permet à l'agent de se concentrer sur l'intention créative plutôt que sur la mécanique d'exécution.

Le DSL adopte le **pattern snapshot+refs** popularisé par agent-browser  [(paddo.dev)](https://paddo.dev/blog/agent-browser-context-efficiency/)  : chaque entité créée reçoit un identifiant stable sous forme de **référence** (`@v1`, `@a1`, `@i1`, `@fx1`). Ces réfs permettent à l'agent de manipuler des objets complexes par de simples noms symboliques, sans avoir à répéter des chemins de fichiers ou des UUID opaques. Le système de **snapshot** fournit une représentation condensée de l'état courant — par exemple, la liste de toutes les réfs avec leur type et leur état — permettant à l'agent de comprendre le contexte en un coup d'œil, avec une réduction de contexte comparable aux **93% revendiqués par agent-browser**  [(paddo.dev)](https://paddo.dev/blog/agent-browser-context-efficiency/) .

### 2.2 Grammaire du DSL : Commandes par Domaine

Le DSL est organisé en **namespaces** correspondant aux cinq capacités du moteur. Chaque namespace expose un ensemble de verbes qui reflètent le vocabulaire naturel du montage créatif. La syntaxe générale suit le pattern : `aether <namespace> <verb> [args] [--flags]` ou, en mode interactif : `>> <verb> [args]`.

#### 2.2.1 Video — Montage et Compositing

| Commande | Exemple | Description |
|----------|---------|-------------|
| `import` | `import video "intro.mp4" as @v1` | Importe un fichier vidéo dans le projet |
| `trim` | `trim @v1 0:00:03 to 0:00:15` | Découpe un clip sur la timeline  [(betterprogramming.pub)](https://betterprogramming.pub/how-video-editors-implement-timeline-filmstrips-using-ffmpeg-and-javascript-a4683ddaeb3c)  |
| `split` | `split @v1 at 0:00:10 --into @v1a @v1b` | Divise un clip en deux parties |
| `concat` | `concat @v1 @v2 @v3 --transition crossfade:0.5` | Assemble des clips avec transitions |
| `overlay` | `overlay @i1 on @v2 --pos 100,50 --scale 0.3` | Superpose une image sur une vidéo  [(Stack Overflow)](https://stackoverflow.com/questions/75004475/ffmpeg-libav-libavfilter-etx-modify-frame-with-image-or-text-using-c-c-a)  |
| `speed` | `speed @v1 --to 1.5 --ramp ease_in_out` | Change la vitesse avec courbe d'accélération |
| `stabilize` | `stabilize @v1 --method gyro --strength 0.8` | Stabilisation vidéo |
| `colorgrade` | `colorgrade @v1 --lut "film_look.cube" --lift 0.02` | Étalonage colorimétrique |
| `chromakey` | `chromakey @v1 --key #00ff00 --tolerance 0.15` | Incrustation (green screen) |
| `render` | `render @timeline --format mp4 --codec h265 --quality high` | Export final  [(Shotstack)](https://shotstack.io/learn/automating-video-editing/)  |

#### 2.2.2 Audio — Traitement et Mixage

| Commande | Exemple | Description |
|----------|---------|-------------|
| `import` | `import audio "voix.wav" as @a1` | Importe un fichier audio |
| `trim` | `trim @a1 0:00:05 to 0:01:30` | Découpe un segment audio |
| `normalize` | `normalize @a1 --lufs -14 --true_peak -1` | Normalisation au standard broadcast  [(Github)](https://github.com/wizenheimer/vibestudio)  |
| `compress` | `compress @a1 --ratio 3:1 --threshold -18db` | Compression dynamique |
| `eq` | `eq @a1 --low_shelf 80Hz:+3db --high_pass 60Hz` | Égalisation paramétrique |
| `reverb` | `reverb @a1 --room_size medium --wet 0.25` | Réverbération |
| `mix` | `mix @a1 @a2 @a3 --levels 0.8,0.6,0.4 --pan center,left20,right20` | Mixage multi-pistes |
| `sync` | `sync @a1 to @v1 --method waveform` | Synchronisation audio/vidéo |
| `stem_separate` | `stem_separate @a1 --stems vocals,drums,bass,other` | Séparation de sources (Demucs via plugin) |

#### 2.2.3 Image — Illustration et Traitement

| Commande | Exemple | Description |
|----------|---------|-------------|
| `import` | `import image "logo.png" as @i1` | Importe une image raster ou vectorielle  [(Lib.rs)](https://lib.rs/multimedia/images)  |
| `create` | `create canvas 1920x1080 --color #1a1a2e as @i2` | Crée un canevas vierge |
| `draw` | `draw text "Titre" on @i2 --font "Inter-Bold" --size 72 --pos center` | Dessine du texte vectoriel  [(Docs.rs)](https://docs.rs/crate/resvg/latest)  |
| `filter` | `filter @i1 --blur gaussian:5px --contrast 1.2` | Applique des filtres |
| `composite` | `composite @i2 over @i1 --blend multiply --opacity 0.7` | Compositing avec blend modes  [(Github)](https://github.com/linebender/tiny-skia)  |
| `export` | `export @i2 --format png --dpi 300` | Export raster  [(Lib.rs)](https://lib.rs/multimedia/images)  |
| `export_svg` | `export_svg @i2 --file "illustration.svg"` | Export vectoriel  [(Docs.rs)](https://docs.rs/crate/resvg/latest)  |

#### 2.2.4 Animation — Motion et Keyframes

| Commande | Exemple | Description |
|----------|---------|-------------|
| `create_layer` | `create_layer "motion" on @anim1 --type null as @l1` | Crée un calque d'animation |
| `keyframe` | `keyframe @l1.position at 0s --value [0,0] --easing ease_out` | Pose une keyframe  [(Github)](https://github.com/zimond/lottie-rs)  |
| `tween` | `tween @l1.position from 0s to 3s --values [0,0] [1920,0] --easing cubic_bezier` | Interpolation automatique |
| `lottie_import` | `lottie_import "animation.json" as @lottie1` | Importe une animation Lottie  [(Github)](https://github.com/zimond/lottie-rs)  |
| `lottie_export` | `lottie_export @anim1 --file "export.json"` | Export au format Lottie |
| `render_frames` | `render_frames @anim1 --fps 30 --format png` | Rendu image par image |

#### 2.2.5 Generate — Création par IA

| Commande | Exemple | Description |
|----------|---------|-------------|
| `image` | `generate image "hero_shot" --prompt "Un drone survolant..." --model nano_banana as @g1` | Génère une image via API  [(google.dev)](https://ai.google.dev/gemini-api/docs/interactions/image-generation)  |
| `video` | `generate video "intro_clip" --prompt "Coucher de soleil..." --model veo3 --duration 8s as @g2` | Génère une vidéo via API  [(google.dev)](https://ai.google.dev/gemini-api/docs/video)  |
| `audio` | `generate audio "narration" --text "Bienvenue dans..." --voice "fr-FR-Neural2-A" --model minimax as @g3` | Génère du TTS via API  [(postunreel.com)](https://postunreel.com/blog/minimax-audio-review-guide)  |
| `voice_clone` | `clone voice from "sample.wav" as @voice_custom` | Clonage vocal (10s de sample)  [(postunreel.com)](https://postunreel.com/blog/minimax-audio-review-guide)  |
| `extend` | `extend @g2 --prompt "Le drone plonge..." --duration 4s as @g2_ext` | Extension de vidéo  [(google.dev)](https://ai.google.dev/gemini-api/docs/video)  |
| `variation` | `variation of @g1 --strength 0.7 as @g1_alt` | Génère une variation |

### 2.3 Mode Interactif : Le REPL Créatif

Au-delà des commandes one-shot, AETHER expose un **mode interactif** (REPL) qui transforme l'outil en un environnement de création conversationnel. L'agent peut ouvrir une session avec `aether edit --project mon_film`, puis émettre une séquence de commandes `>>` qui modifient incrémentalement le projet. Chaque commande est immédiatement persistée, et le daemon maintient en mémoire l'état complet de la composition. L'agent peut à tout moment demander un `snapshot` pour obtenir une vue d'ensemble du projet, ou un `status` pour connaître l'état des jobs de rendu en cours.

```bash
$ aether edit --project vlog_tokyo_2026
Session ouverte: vlog_tokyo_2026 (pid: 2847)

>> import video "raw/IMG_4521.MOV" as @tokyo_street
>> import video "raw/IMG_4523.MOV" as @temple_visit
>> import audio "raw/voice_over.wav" as @narration

>> trim @tokyo_street 0:00:02 to 0:00:18
>> trim @temple_visit 0:00:05 to 0:00:25

>> generate image "title_card" \
     --prompt "Tokyo 2026, typographie japonaise, néon, style cyberpunk" \
     --model nano_banana \
     --aspect 16:9 \
     as @title

>> overlay @title on @tokyo_street --pos center --fade_in 1s --duration 3s

>> normalize @narration --lufs -16
>> mix @narration over @tokyo_street --duck -20db --sensitivity 0.3

>> concat @tokyo_street @temple_visit --transition crossfade:0.8
>> snapshot
```

Le système de **snapshot** retourne alors une représentation structurée et ultra-compacte de l'état :

```
SNAPSHOT — Project: vlog_tokyo_2026
Timeline: @t1 (duration 0:00:43)
├─ Track "Video" [V]
│  ├─ @clip1: @tokyo_street (0:00:02-0:00:18) [overlay: @title fade_in:1s]
│  └─ @clip2: @temple_visit (0:00:05-0:00:25) [transition: crossfade:0.8s]
├─ Track "Audio Narration" [A]
│  └─ @clip3: @narration (0:00:00-0:00:43) [normalized, ducked]
Assets:
├─ @tokyo_street: Video (4K, 23.976fps, 16s) [trimmed]
├─ @temple_visit: Video (4K, 23.976fps, 20s) [trimmed]
├─ @title: Image (1920x1080) [generated, pending]
└─ @narration: Audio (48kHz, stereo, 43s) [normalized]
Pending: @title (Nano Banana 2, est. 8s)
```

---

## 3. Architecture Technique : Le Stack Rust Multimédia

### 3.1 Video Engine : FFmpeg/libav* + wgpu

Le moteur vidéo d'AETHER s'appuie sur **FFmpeg** via les bindings Rust (`ffmpeg-next` ou `ac-ffmpeg`) pour le décodage, l'encodage et la manipulation de flux, complété par **wgpu** pour les opérations de compositing GPU-acceleré en headless  [(DEV Community)](https://dev.to/jaysmito101/high-performance-gpgpu-with-rust-and-wgpu-4l9i) . Cette dualité CPU/GPU permet de couvrir l'ensemble des besoins : FFmpeg gère les formats, les codecs et la lecture/écriture de fichiers (MP4, MOV, ProRes, DNxHR, EXR), tandis que wgpu exécute les opérations de blending, les effets de post-traitement et les transformations géométriques sur le GPU, avec un fallback CPU via `tiny-skia` pour les environnements sans accélération graphique  [(Github)](https://github.com/linebender/tiny-skia) .

La **filtergraph** de FFmpeg, accessible via l'API `libavfilter`, constitue le fondement du pipeline de traitement vidéo  [(FFmpeg)](https://ffmpeg.org/ffmpeg-filters.html) . AETHER génère dynamiquement ces graphs de filtres à partir du DSL de l'agent : une commande `overlay @i1 on @v2 --blend multiply` se traduit en un graph `buffer -> overlay -> buffersink` avec les paramètres de position et de mode de fusion appropriés. Pour les opérations plus complexes — multi-layer compositing, masques alpha, tracking de mouvement — le moteur bascule sur le **Composition Graph Engine** qui utilise wgpu pour un rendu parallèle sur le GPU  [(DEV Community)](https://dev.to/jaysmito101/high-performance-gpgpu-with-rust-and-wgpu-4l9i) .

| Composant | Crate Rust | Fonction | Alternative |
|-----------|------------|----------|-------------|
| Décodage/Encodage | `ffmpeg-next` | Lecture/écriture de tous les formats vidéo | `ac-ffmpeg` |
| Filtergraph | `libavfilter` (FFI) | Chaînes de filtres FFmpeg programmatiques  [(Stack Overflow)](https://stackoverflow.com/questions/75004475/ffmpeg-libav-libavfilter-etx-modify-frame-with-image-or-text-using-c-c-a)  | Custom wgpu shaders |
| Compositing GPU | `wgpu` | Blend modes, transformations, effets en compute shaders  [(DEV Community)](https://dev.to/jaysmito101/high-performance-gpgpu-with-rust-and-wgpu-4l9i)  | `tiny-skia` (CPU) |
| 2D Raster CPU | `tiny-skia` | Rendering 2D de qualité, gradients, patterns  [(Github)](https://github.com/linebender/tiny-skia)  | `raqote` |
| Couleurs | `yuvxyb` | Conversions d'espaces colorimétriques | — |

### 3.2 Audio Engine : Symphonia + rubato + DSP natif

Le moteur audio adopte une approche similaire à celle des DAW professionnels, avec une **timeline audio multi-pistes** où chaque piste peut contenir des clips, des effets (VST3 via wrappers), et des automations de paramètres. Le décodage est assuré par **Symphonia**, la bibliothèque Rust pure qui supporte FLAC, MP3, MP4, Vorbis, WAV et plus encore  [(Lib.rs)](https://lib.rs/multimedia/audio) . Le **resampling** de haute qualité utilise **rubato** (asynchrone, interpolations variées) pour l'alignement temporel des sources à fréquences d'échantillonnage différentes  [(Lib.rs)](https://lib.rs/multimedia/audio) . L'encodage WAV utilise **hound**, et l'encodage MP3/AAC passe par les codecs FFmpeg.

Le pipeline DSP (Digital Signal Processing) est implémenté en Rust natif, avec une chaîne d'effets modulaire : égalisation paramétrique, compression, réverbération, delay, chorus, et noise reduction. Chaque effet est un **nœud** dans un graph de traitement qui peut être reconfiguré dynamiquement via le DSL. Le mixage final supporte les configurations stéréo, 5.1 surround, et même les formats immersifs ambisoniques via la crate `ambisonic`  [(Lib.rs)](https://lib.rs/multimedia/audio) .

| Composant | Crate Rust | Fonction |
|-----------|------------|----------|
| Décodage | `symphonia` | Lecture FLAC, MP3, AAC, WAV, OGG  [(Docs.rs)](https://docs.rs/rodio)  |
| Resampling | `rubato` | Resampling async haute qualité  [(Lib.rs)](https://lib.rs/multimedia/audio)  |
| Encodage WAV | `hound` | Écriture WAV  [(Lib.rs)](https://lib.rs/multimedia/audio)  |
| DSP | Custom + `dasp` | Chaîne d'effets audio, filtres, oscillateurs |
| Spatialisation | `ambisonic` | Audio 3D/ambisonique  [(Lib.rs)](https://lib.rs/multimedia/audio)  |
| MIDI | `midir` | Entrées/sorties MIDI  [(Lib.rs)](https://lib.rs/multimedia/audio)  |

### 3.3 Image Engine : image-rs + resvg + tiny-skia

Le moteur image couvre à la fois le **raster** et le **vectoriel**. Pour le raster, **image-rs** est la bibliothèque de référence de l'écosystème Rust, supportant la lecture/écriture de PNG, JPEG, TIFF, WebP, AVIF et les opérations de base (redimensionnement, recadrage, rotation, filtres)  [(Lib.rs)](https://lib.rs/multimedia/images) . Pour le vectoriel, **resvg** offre un rendu SVG de qualité professionnelle en pure Rust, avec un préprocesseur **usvg** qui simplifie les fichiers SVG complexes en une forme intermédiaire facilement manipulable  [(Docs.rs)](https://docs.rs/crate/resvg/latest) . Le rendu 2D est accéléré par **tiny-skia**, un port Rust optimisé du moteur Skia qui supporte les formes, les gradients, les patterns et le blending  [(Github)](https://github.com/linebender/tiny-skia) .

L'API de dessin d'AETHER s'inspire de celle d'Illustrator et de Skia : l'agent peut créer des formes primitives (rectangles, ellipses, chemins Bézier), appliquer des remplissages (couleur unie, gradient linéaire/radial, pattern), des traits (épaisseur, jointures, pointillés), et des effets (ombre portée, flou gaussien, glow). Chaque opération est non-destructive et fait partie d'un graph de composition qui peut être ré-édité ou exporté à tout moment.

| Composant | Crate Rust | Fonction |
|-----------|------------|----------|
| Raster I/O | `image` | Lecture/écriture PNG, JPEG, TIFF, WebP  [(Lib.rs)](https://lib.rs/multimedia/images)  |
| Redimensionnement | `fast_image_resize` | Resize SIMD ultra-rapide  [(Lib.rs)](https://lib.rs/multimedia/images)  |
| Rendu SVG | `resvg` / `usvg` | Parsing et rendu SVG complet  [(Docs.rs)](https://docs.rs/crate/resvg/latest)  |
| 2D Rendering | `tiny-skia` | Formes, gradients, patterns, blending  [(Github)](https://github.com/linebender/tiny-skia)  |
| Couleurs | `palette` | Gestion d'espaces colorimétriques, interpolation  [(Lib.rs)](https://lib.rs/multimedia/images)  |
| Polices | `fontdb` + `ttf-parser` | Chargement et rendu de polices |

### 3.4 Animation Engine : lottie-rs + keyframes natifs

Le moteur d'animation supporte deux paradigmes : le **rigging keyframe** natif (inspiré d'After Effects) et le **format Lottie** pour l'interopérabilité. La crate **lottie-rs** permet d'importer, modifier et exporter des animations Lottie JSON en pure Rust, avec un rendu headless vers des séquences d'images (PNG, WebP) ou des vidéos  [(Github)](https://github.com/zimond/lottie-rs) . Le système de keyframes natif offre un contrôle granulaire sur les propriétés animables : position, échelle, rotation, opacité, et toute propriété personnalisée d'un effet. Les courbes d'interpolation supportent les easing prédéfinis (linear, ease_in, ease_out, ease_in_out) et les courbes de Bézier cubiques personnalisées.

| Composant | Crate Rust | Fonction |
|-----------|------------|----------|
| Lottie I/O | `lottie-rs` | Parse/render/export Lottie JSON  [(Github)](https://github.com/zimond/lottie-rs)  |
| Core math | `nannou_core` | Mathématiques animation, courbes, vecteurs  [(Github)](https://github.com/nannou-org/nannou)  |
| Easing | Custom | Interpolations linéaires, Bézier, élastiques |
| Raster headless | `lottie-rs` (headless) | Export séquence d'images  [(Github)](https://github.com/zimond/lottie-rs)  |

---

## 4. Persistance : Le Format de Projet .aether

### 4.1 Pourquoi la Persistance est Critique

Sans persistance, un outil headless est réduit à un exécuteur de commandes one-shot, incapable de supporter les workflows créatifs itératifs qui caractérisent le montage professionnel. Un agent IA qui réalise un montage doit pouvoir : **ouvrir** un projet existant, **modifier** un clip déjà placé, **annuler** une opération, **forker** une version alternative, et **collaborer** avec d'autres agents sur le même projet. AETHER résout ce problème avec un format de projet structuré qui combine une **base SQLite** pour les métadonnées et relations, et des **fichiers JSON** pour les données volumineuses (graphs de composition, timelines, keyframes).

Le format est **inspiré d'OpenTimelineIO** (OTIO) de Pixar  [(Larry Jordan)](https://larryjordan.com/articles/opentimelineio-now-supported-on-final-cut-premiere-and-resolve/) , un standard ouvert d'interchange de données éditoriales utilisé par DaVinci Resolve, Adobe Premiere Pro et Avid Media Composer. OTIO fournit un modèle de données robuste pour les timelines, les clips, les transitions et les références médias, sans embarquer les fichiers médias eux-mêmes — exactement ce qu'il faut pour un outil headless. AETHER étend ce modèle avec des concepts propres au compositing node-based (après Natron/Nuke)  [(NATRON)](https://natrongithub.github.io/)  et à la génération IA (métadonnées de génération, prompts, paramètres).

### 4.2 Structure du Format .aether

Un projet AETHER est un dossier portant l'extension `.aether` contenant :

```
mon_projet.aether/
├── project.db          # SQLite : assets, relations, métadonnées
├── timeline.json       # Structure de timeline (format OTIO étendu)
├── composition.json    # Graph de composition node-based
├── keyframes.json      # Données d'animation et courbes
├── settings.json       # Paramètres du projet (résolution, FPS, couleurs)
├── assets/             # Fichiers médias importés (liens symboliques ou copies)
│   ├── raw/
│   ├── generated/
│   └── cache/
└── renders/            # Exports et fichiers de rendu intermédiaire
```

La base **SQLite** stocke les tables fondamentales : `assets` (tous les médias avec leurs réfs, chemins, hashes, métadonnées techniques), `clips` (les occurrences de médias sur la timeline avec in/out points), `tracks` (pistes vidéo/audio avec leurs propriétés), `effects` (effets appliqués avec paramètres), `nodes` (nœuds du graph de composition), et `generations` (historique des générations IA avec prompts, modèles, résultats). Cette structure relationnelle permet des requêtes complexes comme "trouver tous les clips qui utilisent des assets générés par IA" ou "lister les effets qui consomment le plus de ressources de rendu".

Le fichier **timeline.json** adopte la structure hiérarchique d'OTIO : une `Timeline` contient des `Track`, qui contiennent des `Clip` ou des `Transition`, qui référencent des `MediaReference` externes  [(OpenTimelineIO)](https://opentimelineio.readthedocs.io/en/latest/) . Cette compatibilité permet d'importer/exporter vers DaVinci Resolve, Premiere Pro ou Final Cut Pro via les adaptateurs OTIO existants  [(Larry Jordan)](https://larryjordan.com/articles/opentimelineio-now-supported-on-final-cut-premiere-and-resolve/) . Le fichier **composition.json** encode le graph node-based où chaque nœud est une opération (lecture, transformation, filtre, blend, sortie) et chaque connexion est un flux de données entre nœuds — directement inspiré de l'architecture de Natron  [(NATRON)](https://natrongithub.github.io/) .

| Entité | Format | Description | Inspiration |
|--------|--------|-------------|-------------|
| Timeline | JSON (OTIO) | Structure éditoriale clips/tracks/transitions  [(OpenTimelineIO)](https://opentimelineio.readthedocs.io/en/latest/)  | OpenTimelineIO  [(PyPI)](https://pypi.org/project/OpenTimelineIO/)  |
| Composition | JSON (graph) | Nœuds et connexions du pipeline de rendu | Natron/Nuke  [(NATRON)](https://natrongithub.github.io/)  |
| Keyframes | JSON | Courbes d'animation, easing, propriétés animées | After Effects |
| Assets | SQLite | Réfs, chemins, métadonnées, hashes | DaVinci Resolve |
| Générations | SQLite | Prompts, modèles, IDs de jobs IA, statuts | — |

### 4.3 Historique, Undo/Redo, et Versioning

Chaque commande DSL qui modifie le projet est enregistrée dans une table `history` de la base SQLite avec un hash de l'état précédent, permettant un **undo/redo illimité** et même un **time-travel** vers n'importe quel état passé du projet. Cette approche, similaire à celle des systèmes de contrôle de version, permet à l'agent de tester des variations créatives sans crainte de perdre le travail précédent. La commande `aether branch --create "version_alternative"` crée une branche indépendante du projet, et `aether merge` permet de fusionner des modifications entre branches — des concepts directement empruntés à Git, adaptés au workflow créatif.

---

## 5. AI Generation Layer : Plugins Asynchrones pour APIs Externes

### 5.1 Architecture Plugin

La cinquième capacité d'AETHER — la génération IA d'images, vidéos et audio — est implémentée via un **système de plugins asynchrones** qui encapsulent chaque API externe. Chaque plugin implémente un trait Rust commun `AIGenerator` avec les méthodes `generate()`, `status()`, `cancel()`, et `download()`. Cette abstraction permet d'ajouter de nouveaux fournisseurs IA sans modifier le cœur du moteur. Les plugins sont chargés dynamiquement au démarrage du daemon à partir d'un dossier `plugins/`, et leur configuration (clés API, endpoints, quotas) est stockée dans le fichier `settings.json` du projet.

Lorsqu'un agent émet une commande `generate video ...`, le daemon : (1) valide les paramètres, (2) sélectionne le plugin approprié selon le modèle demandé, (3) soumet la requête à l'API avec retry et backoff exponentiel, (4) retourne immédiatement une référence `@gx` avec le statut `pending`, et (5) poll en arrière-plan l'état du job. L'agent peut interroger le statut via `status @gx` ou recevoir une notification automatique lorsque la génération est terminée. L'asset généré est automatiquement importé dans le projet et prêt à être utilisé dans la timeline ou la composition.

### 5.2 Intégration Google Veo 3.1

L'API Veo 3.1 de Google, accessible via la **Gemini API** (`veo-3.1-generate-preview`), supporte la génération de vidéos jusqu'à **8 secondes** à partir d'un prompt texte, avec des options de résolution (720p, 1080p, 4K), de ratio d'aspect (16:9, 9:16, 1:1), et même l'**image-to-video** (génération à partir d'une image de départ ou d'interpolation entre deux images)  [(google.dev)](https://ai.google.dev/gemini-api/docs/video) . Le plugin Veo gère l'asynchronisme natif de l'API (qui retourne un `operation` à poller), le téléchargement depuis Google Cloud Storage, et l'intégration automatique dans le projet AETHER  [(veo3ai.io)](https://www.veo3ai.io/blog/veo-3-api-integration-guide-2026) . L'agent peut également utiliser l'**extension de vidéo** (video-to-video) pour allonger un clip généré en chaînant plusieurs requêtes  [(google.dev)](https://ai.google.dev/gemini-api/docs/video) .

```rust
// Exemple de requête Veo 3.1 via le plugin AETHER
generate video "sunset_drone" \
    --prompt "Un drone survolant un canyon au coucher de soleil, lumière dorée, mouvement lent et cinématique" \
    --model veo3 \
    --resolution 1080p \
    --aspect 16:9 \
    --duration 8s \
    as @sunset_clip
```

### 5.3 Intégration MiniMax Speech 2.5

L'API **MiniMax Speech 2.5** offre une synthèse vocale de haute qualité supportant **plus de 40 langues** avec un **clonage vocal instantané** à partir de seulement **10 secondes** d'échantillon audio  [(postunreel.com)](https://postunreel.com/blog/minimax-audio-review-guide) . Le plugin MiniMax expose ces capacités via le DSL avec des contrôles granulaires sur la vitesse (0.5x à 2.0x), le volume (0-10), la hauteur tonale (-12 à +12), et l'émotion (happy, sad, angry, calm, surprised...)  [(ai documentation)](https://reference-server.pipecat.ai/en/latest/api/pipecat.services.minimax.tts.html) . L'API supporte à la fois le mode batch (pour les narrations longues) et le mode streaming (pour les applications interactives avec une latence < 250ms)  [(postunreel.com)](https://postunreel.com/blog/minimax-audio-review-guide) . Le plugin gère automatiquement la segmentation des textes longs, la gestion des quotas, et le fallback vers des voix alternatives en cas d'indisponibilité.

| API | Capacité | Modèle | Durée/Résolution | Plugin AETHER |
|-----|----------|--------|------------------|---------------|
| **Google Veo 3.1** | Vidéo T2V, I2V, extension | `veo-3.1-generate-preview`  [(google.dev)](https://ai.google.dev/gemini-api/docs/video)  | 8s, 720p-4K | `veo3-plugin` |
| **Nano Banana 2** | Image T2I, édition, variation | `gemini-3.1-flash-image`  [(google.dev)](https://ai.google.dev/gemini-api/docs/image-generation)  | Jusqu'à 2K | `nano_banana-plugin` |
| **Wan 2.5** | Vidéo T2V + audio natif | `wan2.5-t2v-preview`  [(kie.ai)](https://kie.ai/wan-2-5)  | 720p/1080p | `wan25-plugin` |
| **MiniMax Speech** | TTS, clonage vocal, STT | `speech-2.5`, `t2a_v2`  [(Minimax Ai)](https://minimax-ai.chat/models/minimax-speech-25/)  | Variable | `minimax-plugin` |

---

## 6. L'Interface Agent : MCP, Snapshots, et le Pattern Ref

### 6.1 Intégration MCP (Model Context Protocol)

AETHER s'intègre nativement au **Model Context Protocol (MCP)** d'Anthropic, le standard émergent qui permet aux agents IA de découvrir et d'utiliser des outils externes via une interface unifiée  [(modelcontextprotocol.io)](https://modelcontextprotocol.io/docs/getting-started/intro) . En exposant ses capacités comme un **MCP Server**, AETHER permet à n'importe quel agent compatible (Claude Desktop, Cursor, Copilot, etc.) de découvrir automatiquement les commandes disponibles, leurs paramètres, et leurs types de retour. L'agent n'a pas besoin de connaître la syntaxe du DSL à l'avance — il la découvre dynamiquement via l'introspection MCP et génère les commandes appropriées  [(Medium)](https://medium.com/@harshal.dhandrut/building-intelligent-ai-agents-with-mcp-a-complete-guide-to-the-model-context-protocol-5507069068fb) .

Cette intégration suit le pattern `mcp-agent` de LastMile AI  [(Github)](https://github.com/lastmile-ai/mcp-agent)  : le daemon AETHER démarre un serveur MCP Streamable HTTP sur un port local, expose ses outils via le standard JSON-RPC de MCP, et reçoit les appels de l'agent. Chaque outil MCP correspond à une commande DSL (par exemple, `aether_video_import`, `aether_audio_mix`, `aether_generate_image`), avec une description sémantique, des schémas de paramètres JSON Schema, et des exemples d'utilisation. L'agent peut lister les outils, appeler un outil avec des arguments, et recevoir le résultat structuré — le tout via le protocole standardisé  [(openai.github.io)](https://openai.github.io/openai-agents-python/mcp/) .

### 6.2 Le Système Snapshot : Contexte Minimal, Utilité Maximale

Le système de **snapshot** est l'innovation la plus directement inspirée de **agent-browser**  [(Github)](https://github.com/vercel-labs/agent-browser) . Lorsqu'un agent a besoin de comprendre l'état courant du projet pour prendre une décision, il n'a pas à parser des fichiers JSON complexes ou à exécuter des requêtes SQL. Il émet simplement `aether snapshot`, et le daemon retourne une représentation **sémantique condensée** de l'état — une sorte de "résumé créatif" que le LLM peut comprendre en une seule lecture.

Contrairement à un dump brut de la base de données qui pourrait contenir des centaines de lignes de métadonnées techniques, le snapshot ne retient que l'information **actionnable** : la liste des assets avec leur type et statut, la structure de la timeline avec les durées, les opérations en attente, et les éventuels problèmes (assets manquants, jobs échoués, conflits de versions). Cette approche réduit la consommation de tokens de contexte d'environ **90-93%** par rapport à une représentation brute, tout en conservant l'intégralité de l'information nécessaire à la prise de décision  [(paddo.dev)](https://paddo.dev/blog/agent-browser-context-efficiency/) .

Le système de **refs** (`@v1`, `@a1`, `@fx1`) fonctionne en synergie avec le snapshot : au lieu d'identifier les entités par des UUID opaques ou des chemins de fichiers longs, le snapshot utilise les réfs courtes et mémorisables. L'agent voit `@v1: Video "intro.mp4" (trimmed 0:00:02-0:00:15)` plutôt que `asset_7f3a9b2e-4d1c-4f8a-9e5d-1c2b3a4d5e6f : /home/user/projects/mon_projet/assets/raw/intro.mp4 [in: 00:00:02.000, out: 00:00:15.000]`. Cette lisibilité est fondamentale pour que l'agent puisse raisonner efficacement sur le projet.

### 6.3 Le Workflow Agent Complet

Le flux de travail type d'un agent IA utilisant AETHER s'articule en boucles itératives de **planification → exécution → observation → ajustement**. L'agent commence par analyser le brief créatif et planifier la structure du projet (durée, nombre de séquences, type de médias nécessaires). Il initialise le projet avec `aether init`, importe les assets existants, et génère par IA ceux qui manquent. À chaque étape, il prend un snapshot pour vérifier l'état, puis itère sur les ajustements jusqu'à obtenir le résultat souhaité. Le rendu final est déclenché par une commande `render`, et l'agent peut vérifier le résultat avant livraison.

![Flux de Travail Agent](aether_agent_workflow.png)

---

## 7. Comparaison avec l'Écosystème Existant

### 7.1 Outils Headless de Montage Vidéo

| Outil | Langage | Type | Timeline | Compositing | IA Gen | Persistence | Agent-Ready |
|-------|---------|------|----------|-------------|--------|-------------|-------------|
| **FFmpeg CLI** | C | Outil bas niveau | ❌ Filtergraph manuel  [(FFmpeg)](https://ffmpeg.org/ffmpeg-filters.html)  | ❌ Limité | ❌ | ❌ | ❌ |
| **auto-editor** | Python | Script d'automatisation | ✅ Basique  [(Github)](https://github.com/clavesi/ffmpeg-automated-editor)  | ❌ | ❌ | ❌ | ⚠️ |
| **VibeStudio** | Bash | MCP Server FFmpeg | ⚠️ Via FFmpeg  [(Github)](https://github.com/wizenheimer/vibestudio)  | ❌ | ❌ | ❌ | ✅ MCP |
| **Shotstack API** | Cloud | API managée | ✅ Simple  [(Shotstack)](https://shotstack.io/learn/automating-video-editing/)  | ❌ | ❌ | Cloud | ✅ API |
| **Text-to-Clip (AWS)** | Cloud | Pipeline ML | ✅ Automatique  [(MCP Servers)](https://mcpmarket.com/zh/server/vibestudio)  | ❌ | ✅ (script→vidéo) | Cloud | ✅ API |
| **AETHER** | Rust | CLI+Daemon+DSL | ✅ Professionnelle | ✅ Node-based | ✅ Multi-API | ✅ SQLite+JSON | ✅ MCP+Snapshot |

### 7.2 Outils d'Agents IA (Inspiration Architecture)

| Outil | Fonction | Innovation Clé | Ce qu'AETHER en Retient |
|-------|----------|---------------|------------------------|
| **agent-browser**  [(Github)](https://github.com/vercel-labs/agent-browser)  | Navigation web pour agents | Snapshot+refs (-93% contexte) | Pattern référence + snapshot condensé |
| **VibeStudio**  [(Github)](https://github.com/wizenheimer/vibestudio)  | Vidéo headless via MCP | Bash pur, zero runtime | Intégration MCP standardisée |
| **mcp-agent**  [(Github)](https://github.com/lastmile-ai/mcp-agent)  | Framework agents MCP | Patterns composables | Architecture plugin MCP |
| **Omniflow**  [(MCP Servers)](https://mcpmarket.com/zh/server/vibestudio)  | Pipeline créatif IA | Orchestration multi-API | Gestion des jobs asynchrones IA |

---

## 8. Feuille de Route Technique

### 8.1 Phase 1 : Fondations (MVP Rust)

| Milestone | Composant | Crates Rust | Durée estimée |
|-----------|-----------|-------------|---------------|
| **CLI Parser** | `aether-cli` avec clap + REPL | `clap`, `rustyline`, `serde` | 2 semaines |
| **Daemon Core** | `aether-daemon` tokio + UDS | `tokio`, `tonic`, `tarpc` | 3 semaines |
| **Video Engine v1** | Import, trim, concat via FFmpeg | `ffmpeg-next`, `ac-ffmpeg` | 3 semaines |
| **Audio Engine v1** | Import, trim, normalize | `symphonia`, `hound`, `rubato` | 2 semaines |
| **Image Engine v1** | Import, create canvas, draw text | `image`, `resvg`, `tiny-skia` | 2 semaines |
| **Persistence** | Format .aether, SQLite, JSON | `rusqlite`, `serde_json` | 2 semaines |
| **Snapshot+Refs** | Système de réfs + snapshot agent | Custom | 1 semaine |

### 8.2 Phase 2 : Création Professionnelle

| Milestone | Description | Dépendances |
|-----------|-------------|-------------|
| **Composition Node-Based** | Graph engine wgpu/tiny-skia | `wgpu`, `tiny-skia` |
| **Transitions & Effects** | Crossfade, dissolve, wipe, blur | FFmpeg filters + custom shaders |
| **Audio DSP** | EQ, compresseur, réverb, mixage | Custom DSP + `dasp` |
| **Keyframes & Animation** | Système keyframe natif | `nannou_core`, `lottie-rs` |
| **Lottie I/O** | Import/export Lottie | `lottie-rs`  [(Github)](https://github.com/zimond/lottie-rs)  |

### 8.3 Phase 3 : IA Génération & Intégration Agent

| Milestone | API Intégrée | Plugin |
|-----------|--------------|--------|
| **Génération Image** | Nano Banana 2  [(google.dev)](https://ai.google.dev/gemini-api/docs/image-generation)  | `aether-plugin-nano-banana` |
| **Génération Vidéo** | Veo 3.1  [(google.dev)](https://ai.google.dev/gemini-api/docs/video)  | `aether-plugin-veo3` |
| **Génération Vidéo+Audio** | Wan 2.5  [(kie.ai)](https://kie.ai/wan-2-5)  | `aether-plugin-wan25` |
| **TTS & Clonage** | MiniMax Speech 2.5  [(Minimax Ai)](https://minimax-ai.chat/models/minimax-speech-25/)  | `aether-plugin-minimax` |
| **MCP Server** | Protocole Anthropic  [(modelcontextprotocol.io)](https://modelcontextprotocol.io/docs/getting-started/intro)  | Intégration native daemon |

---

## 9. Conclusion : Vers un Creative OS pour Agents IA

AETHER représente une proposition architecturale ambitieuse : celle d'un **système d'exploitation créatif headless** où les agents IA peuvent réaliser l'intégralité d'un pipeline de production multimédia — de la conception à l'export final — via un langage conçu pour être compris, généré et raisonné par des modèles de langage. En s'appuyant sur l'écosystème Rust pour la performance et la sécurité, en adoptant les patterns éprouvés de l'industrie du montage (timeline éditoriale, node-based compositing, pipeline audio professionnel), et en intégrant les APIs de génération IA les plus avancées, AETHER comble le fossé entre l'imagination créative d'un agent IA et sa capacité de production concrète.

L'architecture CLI-Daemon, le DSL sémantique, le système snapshot+refs, et l'intégration MCP ne sont pas de simples choix techniques : ce sont les **fondations d'une nouvelle interface homme-machine** (ou plutôt, agent-machine) où la créativité peut s'exprimer sans friction. Dans cette vision, AETHER n'est pas qu'un outil — c'est le **canal** par lequel les agents IA donnent forme au monde.
