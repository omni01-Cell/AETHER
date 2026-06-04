# aether-api-bridge

Couche **TypeScript** pour les appels API hétérogènes (OpenAI, Google Gemini, etc.). Le runtime Rust (`aether-generate`) communique via **JSON sur stdin/stdout** — un processus Node par requête.

## Build

```bash
cd services/aether-api-bridge
npm install
npm run build
```

Variables d'environnement (jamais dans le repo) :

- `AETHER_OPENAI_API_KEY` ou `OPENAI_API_KEY`
- `AETHER_GOOGLE_API_KEY`, `GEMINI_API_KEY`, ou `GOOGLE_API_KEY`
- `AETHER_API_BRIDGE_SCRIPT` — chemin vers `dist/index.js` (optionnel)
- `AETHER_NODE` — binaire node (défaut : `node`)

## Édition d'image (`image_edit`)

| Rôle | ID AETHER | Fichier bridge |
|------|-----------|----------------|
| Principal | `openai/gpt-image-2` | `src/providers/openai-image-edit.ts` → [GPT Image 2](https://developers.openai.com/api/docs/models/gpt-image-2) |
| Fallback | `google/gemini-3.1-flash-image-preview` | `src/providers/nano-banana-image-edit.ts` (Nano Banana 2) |
| Alias fallback | `google/nano-banana-2` | même handler Nano Banana |

Si GPT Image 2 échoue, le runtime Rust relance le **prompter** avec le guide Nano Banana puis appelle ce provider.

### OpenAI — paramètres (`options.openai`)

Référence : [Images Edit API](https://developers.openai.com/api/reference/resources/images/methods/edit)

- `api_model` : `gpt-image-1`, `gpt-image-1.5`, `gpt-image-1-mini`, `chatgpt-image-latest`
- `quality` : `low` | `medium` | `high` | `auto`
- `size` : `auto` | `1024x1024` | `1536x1024` | `1024x1536`
- `n`, `output_format`, `output_compression`, `background`, `input_fidelity`, `moderation`, `mask_path`

### Nano Banana — paramètres (`options.google`, remplis par le prompter)

Handler : `nano-banana-image-edit.ts` — modèle API `gemini-3.1-flash-image-preview`.

### Google / Gemini — paramètres (`options.google`)

Référence : [Gemini 3.1 Flash Image Preview](https://ai.google.dev/gemini-api/docs/models/gemini-3.1-flash-image-preview)

- `api_model` : `gemini-3.1-flash-image-preview`
- `aspect_ratio` : `1:1`, `16:9`, `9:16`, `4:3`, `3:4`, `21:9`, `1:4`, `4:1`, `1:8`, `8:1`
- `image_size` : `0.5K`, `1K`, `2K`, `4K`
- `temperature`, `top_p`

## Protocole IPC (Rust → Node)

Requête (stdin) :

```json
{
  "version": 1,
  "operation": "image_edit",
  "model_id": "openai/gpt-image-1-medium",
  "prompt": "Add watercolor effect",
  "input_image_paths": ["/abs/path/source.png"],
  "output_dir": "/abs/path/out",
  "options": {}
}
```

Réponse (stdout) : `{ "ok": true, "artifacts": [{ "path": "...", "mime_type": "image/png" }] }` ou `{ "ok": false, "error": "...", "retryable": true }`.
