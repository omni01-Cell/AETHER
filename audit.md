# Audit d'incomplétude du projet AETHER

Date de l'audit : 2026-05-29

## Résumé

Le projet AETHER dispose déjà d'une base Rust structurée en workspace avec plusieurs crates (`aether-core`, `aether-cli`, `aether-daemon`, `aether-video`, `aether-audio`, `aether-image`, `aether-generate`, `aether-project`, `aether-vault`, `aether-planner`, etc.). Cependant, l'implémentation actuelle ne couvre pas encore l'ensemble des fonctionnalités annoncées dans le `README.md`.

Le principal écart constaté est que la documentation décrit un moteur multimédia agentique complet, avec providers IA réels, DSL riche, planner avancé et moteurs média étendus, alors que le code contient encore plusieurs briques en mode mock, template ou partiellement exposées.

## 1. Providers IA réels absents ou désactivés

Le `README.md` annonce des intégrations natives avec des réseaux génératifs comme Google Veo, Imagen, Lyria, MiniMax, Wan et Nano Banana.

Dans le code actuel :

- le registre de modèles contient des modèles mock activés ;
- les modèles réels Google sont présents comme placeholders, mais avec `enabled: false` ;
- le runtime par défaut utilise `MockProvider` ;
- aucun client API réel complet n'est branché pour Google, MiniMax, Wan, Veo, Imagen, Lyria ou équivalent.

### Ce qui manque

- Clients API réels pour les providers IA.
- Configuration des clés API et secrets.
- Gestion des erreurs réseau/provider.
- Téléchargement de vrais fichiers image/audio/vidéo.
- Activation configurable des modèles réels.
- Providers MiniMax, Wan, Nano Banana et autres modèles cités dans la documentation.

## 2. Génération média encore principalement mock

Le `MockProvider` écrit des fichiers JSON déterministes pour simuler les résultats de génération :

- storyboard JSON ;
- dialogue JSON ;
- image mock JSON ;
- audio mock JSON ;
- music mock JSON ;
- video mock JSON.

Ensuite, le daemon ignore explicitement les artefacts `.json` ou `.mock-*` lorsqu'il s'agit d'enregistrer des assets média réels.

Conséquence : une commande comme `generate image`, `generate voice`, `generate music` ou `generate video` peut créer un job de génération, mais ne produit pas nécessairement de référence média exploitable du type `@img1`, `@a1` ou `@v1` en mode mock.

### Ce qui manque

- Artefacts média réels générés par défaut ou en mode test.
- Option de mock produisant de petits fichiers valides, par exemple PNG/WAV/MP4 minimaux.
- Enregistrement cohérent des assets générés pour tester les workflows complets.
- Tests end-to-end capables d'utiliser les références produites par génération.

## 3. DSL documenté plus large que le parser CLI réel

Le `README.md` documente de nombreuses commandes avancées :

- `detect-cuts` ;
- `auto-reframe` ;
- `bake` ;
- `strip-silence` ;
- `apply-fades` ;
- `enhance-voice` ;
- `vignette` ;
- `blend-if` ;
- `layer-style` ;
- `analyze-color` ;
- `group-clips` ;
- `move-group` ;
- `lock-time` ;
- `create-adjustment` ;
- `expression set` ;
- `particles create`.

Le parser CLI actuel expose surtout les commandes de base :

- `init` ;
- `import` ;
- `trim` ;
- `mix` ;
- `composite` ;
- `canvas` ;
- `draw_text` ;
- `export` ;
- `inspect` ;
- commandes `generate ...` ;
- commandes `project`, `vault`, `plan` principales.

Les commandes documentées mais non reconnues finissent donc en erreur `Unknown command`.

### Ce qui manque

- Implémenter ou retirer de la documentation les commandes DSL avancées non supportées.
- Ajouter les branches de parsing CLI manquantes.
- Ajouter les handlers daemon correspondants.
- Ajouter les tests unitaires et E2E pour ces commandes.

## 4. Commandes présentes dans le coeur mais non exposées correctement par le CLI

Le type `Command` dans `aether-core` contient déjà plusieurs commandes utiles :

- `Concat` ;
- `Overlay` ;
- `Speed` ;
- `Eq` ;
- `Compress` ;
- `MixTracks` ;
- `KeyframeSet` ;
- `KeyframeList` ;
- `ExportOtio` ;
- `ExportEdl`.

Cependant, le parser CLI ne semble pas exposer toutes ces commandes via une syntaxe utilisateur correspondante.

### Ce qui manque

- Parsing CLI pour `concat`.
- Parsing CLI pour `overlay`.
- Parsing CLI pour `speed`.
- Parsing CLI pour `eq`.
- Parsing CLI pour `compress`.
- Parsing CLI pour `mix-tracks`.
- Parsing CLI pour `keyframe-set` et `keyframe-list`.
- Parsing CLI pour `export-otio` et `export-edl`.
- Documentation synchronisée avec la syntaxe réelle.

## 5. Documentation et syntaxe CLI désalignées

Plusieurs exemples du `README.md` ne correspondent pas exactement à ce que le parser accepte actuellement.

### Exemple : vault create

Documentation :

```bash
aether vault create brand maison_lux_time
```

Parser actuel :

```bash
vault create <name> --kind <kind>
```

### Exemple : vault add

Documentation :

```bash
aether vault add logo maison_lux_time ./logo_primary.png --variant primary --usage generate-image,export-branding
```

Parser actuel :

```bash
vault add <vault_id> <asset_name> --file <path> --type <type> --usage <usage>
```

### Exemple : plan show

Documentation :

```bash
aether plan show
```

Parser actuel :

```bash
plan show <plan_id>
```

### Ce qui manque

- Choisir une syntaxe officielle.
- Mettre à jour soit le parser, soit la documentation.
- Ajouter des tests de parsing couvrant tous les exemples du `README.md`.

## 6. Planner encore template, pas vrai planificateur IA complet

La documentation décrit un planner capable de transformer des briefs créatifs en checklists validées, traçables et réversibles.

Le code actuel crée principalement un template fixe de six étapes :

1. initialiser le canvas ;
2. générer un storyboard ;
3. générer une voix off ;
4. générer une musique ;
5. compiler une vidéo depuis les ingrédients ;
6. exporter le rendu final.

La vérification d'une étape contrôle surtout les dépendances, puis marque l'étape comme terminée avec une preuve optionnelle. La validation physique stricte annoncée reste limitée.

### Ce qui manque

- Génération dynamique de plans selon le brief.
- Commande `plan validate` annoncée mais absente du parser observé.
- Validation des commandes du plan contre le DSL réel.
- Validation physique stricte des preuves avant `plan check`.
- Intégration plus profonde avec les assets, hashes, DB records et sorties média.

## 7. Backend GPU surtout fallback CPU

Le backend GPU initialise potentiellement `wgpu` lorsque la feature GPU est activée, mais retombe ensuite sur `CpuBackend` pour le rendu effectif.

Quand la feature GPU n'est pas activée, le backend renvoie simplement une erreur indiquant que le GPU est désactivé.

### Ce qui manque

- Pipeline GPU réel.
- Shaders ou kernels de rendu.
- Gestion des textures et buffers GPU.
- Tests de rendu GPU ou fallback explicitement documenté.

## 8. Environnement de test incomplet dans le conteneur actuel

La commande suivante a été exécutée :

```bash
cargo test --workspace
```

Elle échoue dans l'environnement actuel pendant la compilation de `ffmpeg-sys-next`, car `pkg-config` ne trouve pas `libavutil.pc`.

Message clé :

```text
Package 'libavutil', required by 'virtual:world', not found
The system library `libavutil` required by crate `ffmpeg-sys-next` was not found.
```

Le `README.md` indique bien que les bibliothèques de développement FFmpeg sont nécessaires.

### Ce qui manque dans l'environnement

- `libavutil` et les fichiers `.pc` associés.
- Paquets de développement FFmpeg, par exemple sur Debian/Ubuntu :
  - `libavcodec-dev` ;
  - `libavformat-dev` ;
  - `libavfilter-dev` ;
  - `libavdevice-dev` ;
  - `libswscale-dev` ;
  - `libswresample-dev`.

## 9. Fonctionnalités média annoncées mais partielles

Les crates média existent et contiennent déjà des fonctions utiles :

- import vidéo ;
- trim vidéo ;
- concat vidéo ;
- render/export vidéo ;
- composite vidéo ;
- import audio ;
- trim audio ;
- normalize audio ;
- EQ ;
- compressor ;
- mix tracks ;
- import image ;
- canvas ;
- draw text ;
- export image.

Cependant, plusieurs fonctionnalités annoncées dans la documentation semblent absentes ou non exposées :

- détection de cuts ;
- auto-reframe ;
- smart crops ou motion tracking ;
- strip silence ;
- transient detection ;
- fades automatisés ;
- enhance voice ;
- vignette ;
- blend-if avancé ;
- layer styles ;
- analyse couleur complète ;
- groupes de clips ;
- adjustment layers ;
- expressions ;
- particules.

## 10. Priorités recommandées

### Priorité 1 : stabiliser la vérité produit

- Décider si le projet doit être présenté comme MVP mock ou comme moteur complet.
- Mettre le `README.md` en phase avec le code actuel.
- Ajouter une section claire `Implemented / Planned`.

### Priorité 2 : rendre les workflows de base réellement bout-en-bout

- Faire produire au mock des médias minimaux valides.
- Garantir que `generate image` donne un `@img1`, `generate voice` donne un `@a1`, etc.
- Ajouter des tests E2E qui enchaînent génération, inspection, mix/composition et export.

### Priorité 3 : exposer les commandes déjà présentes dans `Command`

- Ajouter le parsing pour `concat`, `overlay`, `speed`, `eq`, `compress`, `mix-tracks`, keyframes, OTIO et EDL.
- Ajouter les tests de parser correspondants.
- Vérifier que le daemon gère toutes les commandes exposées.

### Priorité 4 : compléter le planner

- Ajouter `plan validate`.
- Valider les commandes planifiées contre le parser réel.
- Vérifier les preuves physiques avant de cocher une étape.
- Produire des plans adaptés au brief plutôt qu'un template fixe.

### Priorité 5 : providers IA réels

- Concevoir une abstraction de configuration provider.
- Ajouter au moins un provider réel complet en premier.
- Gérer secrets, quotas, erreurs, polling, downloads et retries.
- Conserver `MockProvider` pour les tests déterministes.

### Priorité 6 : environnement CI/test

- Installer/documenter précisément les dépendances FFmpeg système.
- Ajouter éventuellement des features Cargo pour tester les crates non-FFmpeg séparément.
- Ajouter une CI qui vérifie build, fmt, clippy et tests.

## Conclusion

AETHER est une bonne base de moteur multimédia Rust modulaire, mais il reste incomplet par rapport à son ambition documentée.

Le socle existe : crates, types centraux, daemon, CLI, persistence, vault, planner, génération mock et moteurs média de base. Les principaux manques sont les providers IA réels, la production de vrais assets générés, l'exposition complète du DSL, l'alignement documentation/CLI, le planner avancé, le backend GPU réel et l'environnement de test FFmpeg.
