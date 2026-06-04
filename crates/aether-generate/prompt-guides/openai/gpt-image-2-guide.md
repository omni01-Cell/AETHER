# GPT Image 2 (OpenAI)

State-of-the-art image generation and editing.

## Editing

- Prefer **one clear imperative** instruction per edit (e.g. "Change the tie to green", "Remove the car in the background").
- State explicitly what must **remain unchanged** from the source image(s).
- Do not rewrite from scratch if the image is already ~80% correct.

## Quality and output

- Use `quality: high` only when the user demands maximum fidelity or pixel accuracy.
- Request `background: transparent` only when explicitly asked.
- Choose `size` from allowed presets; use `auto` when the user does not specify dimensions.

## Prompt style

- No chat filler — output only the image instruction the API will receive.
- Be specific about subjects, lighting, and composition when generating from text.
