# Prompt guides & routage agents

## Contexte LLM agent (`system.md` + tools + `{model}.json`)

| Partie | Fichier |
|--------|---------|
| System message | `system.md` avec `{model}` |
| Tools | Rust `prompter/tools.rs` + TS via bridge |
| Complément modèle | `{provider}/{name}.json` → `model_guide` inline + `parameters` |

## Appels par agent (Rust → JSON → bridge TS)

| Agent | Fichier Rust | Config JSON |
|-------|--------------|-------------|
| Prompter | `src/agents/prompter_call.rs` | `agents.v1.json` → `"prompter"` |
| Planner | `src/agents/planner_call.rs` | `agents.v1.json` → `"planner"` |

Exemple `agents.v1.json` :

```json
{
  "prompter": {
    "provider": "openai",
    "model_id": "openai/gpt-4o-mini",
    "api_model": "gpt-4o-mini",
    "bridge_handler": "openai-chat"
  }
}
```

`PrompterCall` / `PlannerCall` lisent ce fichier et appellent `services/aether-api-bridge` avec `operation: chat_completions` et `bridge_handler`.

## Génération image (`routing.v1.json`)

| function | primary | bridge_handler |
|----------|---------|----------------|
| edit-image | openai/gpt-image-2 | openai-image-edit |
| edit-image fallback | google/gemini-3.1-flash-image-preview | nano-banana-image-edit |

Le runtime injecte `bridge_handler` dans `options` ; le bridge TS route via `router.ts` (plus de listes hardcodées par défaut).

`AETHER_PROMPT_GUIDES_DIR` — racine des JSON.
