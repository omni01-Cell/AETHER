# A faire - Ordre d'implementation AETHER

## Objectif court terme

Ajouter deux briques creatives au pipeline AETHER :

1. `aether-fetch` : un sous-agent de recherche qui recoit une demande creative, cherche sur internet des assets et informations utiles, puis renvoie soit un asset exploitable, soit un `bloc-info` cite et structure.
2. `aether-generate` + Remotion : une action supplementaire de generation video locale qui rend des compositions React frame par frame, utile pour les montages graphiques, UI motion, reels explicatifs, lower thirds, kinetic typography et videos de marque.

## Agent Prompter (standardise schema v1)

1. [x] Moteur generique `prompter/engine.rs` — zero logique Rust par provider ; tout vient du JSON guide.
2. [x] Schema v1 : `prompting_guide` + `parameters` (defaults, allowed, inference, clarify) + `triggers`.
3. [x] Chargement dynamique `prompt-guides/<provider>/<model>.json` selon `model_id` actif.
4. [x] Contexte `prompter_context` serialise pour futur agent LLM prompter (modele a choisir plus tard).
5. [x] OpenAI **gpt-image-2** (pas gpt-image-1-medium) — guide + registry + bridge.
6. [x] Gemini **gemini-3.1-flash-image-preview** — guide complet (ratio, resolution, thinking, search_grounding, etc.).
7. [x] Stubs guides Veo 3.1 / Minimax video (schema v1, pret a enrichir).
8. [x] Clarifications renvoyees au daemon si parametres `clarify_if_missing` non deduits du texte.
9. [ ] Brancher un LLM comme prompter : recevoir demande texte + JSON guide injecte dans le system prompt.
10. [ ] Enrichir les guides JSON pour chaque tache generate quand USER fixe principal/fallback par modele.

## Etat des fondations

1. [x] Finaliser la conformite `plan2.md`
   - Commandes longues `generate image`, `generate voice`, `generate video`, `generate storyboard-scratch`.
   - Commandes `generation status` et `generation cancel`.
   - Compatibilite avec les alias courts.

2. [x] Implementer la couche projet
   - Commandes `aether project create/open/current/list/close/delete`.
   - Support `--project <name-or-path>`.
   - Isolation de l'etat dans le bon projet actif.

3. [x] Ajouter les tests projet
   - Creation, ouverture, fermeture, reouverture.
   - Deux projets separes.
   - Suppression archivee et suppression forcee securisee.
   - Erreur claire si aucun projet actif.

4. [x] Implementer AETHER Vault
   - Stockage des assets de marque, logos, filigranes, chartes couleur, styles, personas et personnages recurrents.
   - Attachement des Vaults aux projets.
   - Injection du contexte Vault dans `aether-generate`.

## Epic 1 - Implementer `aether-fetch`

### Role du sous-agent

`aether-fetch` doit agir comme un chercheur/curateur de production. Il ne genere pas directement le media final : il collecte, normalise, cite et prepare des ressources pour les autres agents.

Entrees attendues :

- Demande libre : marque, produit, style, niche, reference visuelle, cible UI/design, besoin d'image, besoin d'information.
- Projet actif obligatoire.
- Contexte Vault optionnel : marque, persona, charte, assets deja connus.
- Contraintes : langue, pays, type d'usage, licence souhaitee, format de sortie.

Sorties attendues :

- `asset` : image, logo public, screenshot, reference visuelle, palette, police referencee, document source ou fichier telecharge.
- `bloc-info` : bloc JSON/Markdown structure avec faits, sources, citations courtes, liens, date de collecte et niveau de confiance.
- `direction-design` : synthese exploitable par le prompt-maker avec couleurs, typographies, motifs UI, ton, composition, contraintes et anti-patterns.

### Types a ajouter dans `aether-core`

- `FetchRequest`
  - `query: String`
  - `intent: FetchIntent`
  - `project_ref` ou contexte projet courant
  - `vault_refs: Vec<Ref>` si necessaire
  - `options: serde_json::Value`

- `FetchIntent`
  - `AssetImage`
  - `BrandResearch`
  - `DesignDirection`
  - `UiReference`
  - `MarketInfo`
  - `Mixed`

- `FetchResult`
  - `result_ref: Ref`
  - `kind: FetchResultKind`
  - `title`
  - `summary`
  - `sources`
  - `assets`
  - `confidence`
  - `collected_at_ms`

- `FetchResultKind`
  - `Asset`
  - `InfoBlock`
  - `DesignDirection`

- `FetchSource`
  - `url`
  - `title`
  - `publisher`
  - `retrieved_at_ms`
  - `license`
  - `usage_notes`

### Crate/module propose

Creer `crates/aether-fetch` avec une architecture testable sans reseau reel :

- `FetchAgent`
  - Orchestre la demande.
  - Lit le projet courant et le Vault attache.
  - Retourne `FetchResult`.

- `SearchProvider` trait
  - `search_web(query, options)`.
  - `search_images(query, options)`.
  - Implementation `MockSearchProvider` pour les tests.
  - Implementations reelles plus tard : Tavily, Brave Search, SerpAPI, Bing, provider custom.

- `PageExtractor`
  - Telecharge une page.
  - Extrait titre, meta, texte utile, images candidates.
  - Garde les citations courtes uniquement.

- `AssetDownloader`
  - Telecharge les images ou fichiers autorises.
  - Calcule hash.
  - Stocke dans `.aether/fetch/assets/`.
  - Enregistre source, licence et provenance.

- `DesignDirectionSynthesizer`
  - Transforme plusieurs sources en direction design concise.
  - Produit un bloc compatible avec `PromptContext`.

### Stockage et integration projet

- Ajouter `.aether/fetch/`
  - `fetch_results.jsonl` ou table SQLite dediee.
  - `assets/` pour les fichiers telecharges.
  - `sources/` pour les snapshots de metadata.

- Les assets valides doivent pouvoir etre :
  - enregistres comme asset projet normal,
  - attaches au Vault si l'utilisateur le demande,
  - passes comme input a `aether-generate`.

- Les `bloc-info` doivent pouvoir etre injectes dans :
  - `PromptMakerContext`,
  - `AETHER Planner`,
  - prompts image/video,
  - futures skills.

### CLI/DSL propose

- `aether fetch asset "<query>"`
- `aether fetch brand "<brand or product>"`
- `aether fetch design-direction "<query>"`
- `aether fetch ui "<query>"`
- `aether fetch info "<query>"`
- `aether fetch show <fetch_ref>`
- `aether fetch attach <fetch_ref> --vault <vault_id>`

Aliases acceptables :

- `fetch asset`
- `fetch brand`
- `fetch design`
- `fetch info`

### Daemon et pipeline

- Ajouter un handler `Command::Fetch...` dans `aether-daemon`.
- Refuser l'execution si aucun projet actif n'est resolu.
- Sauvegarder le resultat avant de retourner la reponse.
- Pour les assets telecharges, enregistrer une provenance complete.
- Pour les blocs info, retourner un resume court dans le CLI et garder le detail dans le stockage projet.

### Regles de securite et qualite

- Ne jamais injecter une source web brute dans un prompt sans nettoyage.
- Toujours enregistrer URL, date de collecte et licence/usage quand disponible.
- Marquer explicitement les assets a licence inconnue.
- Par defaut, preferer les sources officielles pour les marques.
- Pour les images, distinguer :
  - reference visuelle,
  - asset utilisable,
  - asset interdit ou licence inconnue.
- Les tests ne doivent pas dependre d'internet : utiliser `MockSearchProvider`.

### Tests a ajouter

- Parsing CLI des commandes `fetch`.
- `FetchAgent` avec provider mock.
- Stockage d'un `bloc-info`.
- Telechargement mock d'un asset image.
- Enregistrement de provenance.
- Injection d'un `FetchResult` dans un contexte de generation.
- Erreur claire si aucun projet actif.

## Epic 2 - Ajouter Remotion comme action video dans `aether-generate`

### Objectif

Ajouter une action locale qui utilise Remotion pour generer des videos avec React, frame par frame. Cette action doit completer les providers text-to-video/image-to-video, pas les remplacer.

Cas d'usage prioritaires :

- Videos UI/app demo.
- Motion design de marque.
- Kinetic typography.
- Reels explicatifs.
- Lower thirds, titres, transitions graphiques.
- Videos construites depuis assets Vault + `bloc-info` + timeline.
- Variantes rapides ou deterministes quand un provider generatif video serait trop imprevisible.

### Design d'architecture

Remotion vit dans l'ecosysteme Node/React. Le coeur AETHER reste Rust. Il faut donc ajouter un pont explicite :

- Crate Rust : `crates/aether-remotion` ou module `remotion` dans `aether-generate`.
- Workspace Node optionnel : `remotion/` ou `tools/remotion/`.
- Runner Rust : lance un binaire Node controle, avec chemins projet explicites.
- Contrat JSON entre Rust et React :
  - `composition_id`
  - `fps`
  - `width`
  - `height`
  - `duration_frames`
  - `assets`
  - `timeline`
  - `brand_context`
  - `info_blocks`
  - `style`
  - `output_path`

### Types a ajouter

- `GenerationKind::VideoRemotion` ou `GenerationKind::VideoReact`
- `GeneratedArtifactKind::Video` reste suffisant pour le rendu final.
- Optionnel :
  - `GeneratedArtifactKind::FrameSequence`
  - `GeneratedArtifactKind::RemotionBundle`
  - `GeneratedArtifactKind::RenderManifest`

Types dedies :

- `RemotionRenderRequest`
  - `composition`
  - `width`
  - `height`
  - `fps`
  - `duration_frames`
  - `inputs: Vec<Ref>`
  - `style`
  - `script`
  - `options`

- `RemotionRenderManifest`
  - `request`
  - `assets`
  - `output_video`
  - `frames_dir`
  - `logs`
  - `render_time_ms`

### Action `generate`

Ajouter une commande DSL :

- `generate remotion-video "<prompt>"`

Options utiles :

- `--from <refs...>` pour images, videos, audio, fetch results ou Vault assets.
- `--composition <name>` pour choisir un template.
- `--fps <n>`
- `--size 1080x1920`
- `--duration <seconds>`
- `--style "<style hint>"`

Exemples :

- `generate remotion-video "reel vertical pour presenter la marque" --from @img1 @g4 --size 1080x1920 --duration 12`
- `generate remotion-video "hero UI motion avec 3 sections produit" --composition ui-demo --from @g2`

### Relation avec le prompt-maker

Le prompt-maker doit produire un plan de composition plutot qu'un simple prompt video :

- scenes/sections,
- texte a afficher,
- timing,
- transitions,
- couleurs,
- typographie,
- layout,
- assets a utiliser,
- contraintes de marque.

Ce plan devient le JSON d'entree pour Remotion.

### Templates Remotion initiaux

Ajouter 3 templates React simples et robustes :

- `BrandReel`
  - Logo, headline, blocs texte, images de marque.
- `UiDemo`
  - Screenshots, callouts, zoom/pan, captions.
- `InfoExplainer`
  - `bloc-info`, chiffres cles, citations courtes, sources en fin de video.

Chaque template doit accepter le meme manifest JSON et ignorer proprement les champs inconnus.

### Runner Remotion

Le runner doit :

- generer un manifest JSON dans `.aether/generate/remotion/<job_id>/manifest.json`,
- copier/resoudre les assets dans un dossier de rendu stable,
- lancer Remotion en mode non interactif,
- produire un MP4 dans `.aether/generate/artifacts/`,
- enregistrer les logs,
- retourner un `GenerationArtifact` video avec metadata :
  - `renderer: remotion`
  - `composition`
  - `fps`
  - `duration_frames`
  - `width`
  - `height`
  - `manifest_path`

### Implementation progressive

1. Ajouter les types core et la commande CLI.
2. Ajouter un runner mock qui ecrit un manifest et un faux artifact video JSON.
3. Brancher le daemon et la persistence generation existante.
4. Ajouter le workspace Remotion minimal.
5. Ajouter un runner reel derriere feature flag ou detection de `node`/`npm`.
6. Ajouter les templates React.
7. Ajouter l'enregistrement comme asset video projet.
8. Ajouter les tests d'integration avec runner mock.
9. Ajouter un test manuel documente pour le rendu reel.

### Tests a ajouter

- Parsing CLI `generate remotion-video`.
- Creation d'un `GenerationRequest` `VideoRemotion`.
- Prompt-maker produit un plan Remotion structure.
- Runner mock produit un artifact.
- Daemon persiste le job.
- Assets `--from` inexistants : erreur claire.
- Assets resolus : manifest contient les chemins locaux corrects.
- Remotion indisponible : erreur actionnable, sans casser les autres generations.

## Epic 3 - Connecter `aether-fetch`, Vault et Remotion

Flux cible :

1. `aether-fetch` collecte une direction design ou des references.
2. Le resultat est stocke comme `bloc-info` ou asset.
3. Le Vault conserve les contraintes de marque stables.
4. `generate remotion-video` recoit refs assets + blocs info + Vault context.
5. Le prompt-maker construit un plan de composition.
6. Remotion rend la video.
7. La video finale est enregistree dans le projet et peut etre montee dans la timeline.

Commandes cible :

- `fetch brand "Acme"` puis `generate remotion-video "video de lancement produit" --from <fetch_ref>`
- `fetch ui "dashboard finance premium"` puis `generate remotion-video "demo UI animee" --from <fetch_ref> @img1`
- `fetch design-direction "marque wellness haut de gamme"` puis `aether fetch attach <fetch_ref> --vault <vault_id>`

## Ordre prioritaire recommande

1. Finaliser les types core `Fetch*` et `VideoRemotion`.
2. Ajouter parsing CLI et commandes daemon avec mocks.
3. Implementer stockage projet pour `fetch`.
4. Implementer `aether-fetch` avec providers mocks.
5. Brancher `FetchResult` dans `PromptMakerContext`.
6. Ajouter runner mock Remotion dans `aether-generate`.
7. Ajouter workspace Remotion reel et templates initiaux.
8. Ajouter tests integration fetch -> generate remotion.
9. Ajouter providers web reels seulement quand les mocks et contrats sont stables.

## DX Agent / CLI (a faire)

### [ ] Corriger le decoupage des arguments shell en mode one-shot

**Contexte (verification 2026-05-21, `draw_text` « made by IA »)** : en mode one-shot, `aether-cli` reconstruit la ligne avec `args.join(" ")`, ce qui supprime les guillemets du shell. Exemple :

```bash
# Cas qui echoue (Invalid size) : le shell passe plusieurs argv, join perd les quotes
aether-cli draw_text @img1 "made by IA" sans-serif 32 720 990

# Contournement agent actuel : un seul argument argv contenant la ligne DSL complete
aether-cli 'draw_text @img1 "made by IA" sans-serif 32 720 990'
```

**A faire** :

1. Documenter dans README / skill agent le contournement (guillemets simples autour de toute la commande DSL).
2. Corriger le CLI pour ne plus dependre de `join(" ")` : soit lire `std::env::args()` jusqu'a la fin sans perdre les espaces internes aux quotes, soit accepter `--cmd '...'` / stdin / fichier `.aethercmd`.
3. Ajouter un test CLI qui verifie `draw_text` avec texte multi-mots sans ambiguite.

**Priorite** : moyenne (bloquant pour agents naifs, trivial pour agents qui passent une seule chaine).

## Regle de travail

Ne pas brancher de providers web reels tant que les contrats `FetchRequest`, `FetchResult`, `RemotionRenderRequest` et les tests mocks ne sont pas stables. Les recherches internet doivent etre tracables, citees et stockees avec provenance avant d'etre utilisees dans une generation.
