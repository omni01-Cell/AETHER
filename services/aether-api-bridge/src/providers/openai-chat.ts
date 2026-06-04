import type { BridgeRequest, BridgeResponse } from "../protocol.js";
import { bridgeError } from "../protocol.js";

const OPENAI_CHAT_URL = "https://api.openai.com/v1/chat/completions";

export async function runOpenAiChat(req: BridgeRequest): Promise<BridgeResponse> {
  const apiKey = process.env.AETHER_OPENAI_API_KEY ?? process.env.OPENAI_API_KEY;
  if (!apiKey?.trim()) {
    return bridgeError("openai", "OPENAI_API_KEY or AETHER_OPENAI_API_KEY is not set", false);
  }

  const apiModel = req.api_model ?? req.model_id.split("/").pop() ?? "gpt-4o-mini";
  const messages = req.messages ?? [];
  if (messages.length === 0) {
    return bridgeError("openai", "chat_completions requires messages", false);
  }

  const body: Record<string, unknown> = {
    model: apiModel,
    messages,
    ...(req.tools ? { tools: req.tools, tool_choice: "auto" } : {}),
    ...(req.options?.temperature != null ? { temperature: req.options.temperature } : {}),
  };

  let res: Response;
  try {
    res = await fetch(OPENAI_CHAT_URL, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
  } catch (e) {
    return bridgeError(
      "openai",
      e instanceof Error ? e.message : String(e),
      true
    );
  }

  const raw = await res.text();
  if (!res.ok) {
    return bridgeError("openai", `HTTP ${res.status}: ${raw.slice(0, 800)}`, res.status >= 500);
  }

  return {
    ok: true,
    provider: "openai",
    provider_job_id: `chat-${Date.now()}`,
    status: "ready",
    artifacts: [],
    metadata: {
      raw_response: raw,
      agent: req.agent,
      api_model: apiModel,
    },
  };
}
