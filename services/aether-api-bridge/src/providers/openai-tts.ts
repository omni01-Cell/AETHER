import fs from "node:fs";
import path from "node:path";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * OpenAI TTS — Voice generation via OpenAI API
 * @see https://platform.openai.com/docs/api-reference/audio/createSpeech
 */

export interface OpenAITTSParams {
  text: string;
  model: string;
  voice: string;
  response_format: string;
  speed: number;
}

const DEFAULTS: OpenAITTSParams = {
  text: "",
  model: "tts-1-hd",
  voice: "alloy",
  response_format: "mp3",
  speed: 1.0,
};

function parseParams(options: Record<string, unknown> | undefined): OpenAITTSParams {
  const o = options ?? {};
  const cap =
    typeof o.openai === "object" && o.openai !== null
      ? (o.openai as Record<string, unknown>)
      : o;

  return {
    text: typeof cap.text === "string" ? cap.text : DEFAULTS.text,
    model: typeof cap.model === "string" ? cap.model : DEFAULTS.model,
    voice: typeof cap.voice === "string" ? cap.voice : DEFAULTS.voice,
    response_format:
      typeof cap.response_format === "string" ? cap.response_format : DEFAULTS.response_format,
    speed:
      typeof cap.speed === "number" && cap.speed >= 0.25 && cap.speed <= 4.0
        ? cap.speed
        : DEFAULTS.speed,
  };
}

export async function runOpenAITTS(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey = process.env.AETHER_OPENAI_API_KEY ?? process.env.OPENAI_API_KEY;
  if (!apiKey?.trim()) {
    return bridgeError("openai-tts", "Missing OPENAI_API_KEY or AETHER_OPENAI_API_KEY", false);
  }

  const params = parseParams(args.options);
  params.text = args.prompt.trim() || params.text;

  if (!params.text) {
    return bridgeError("openai-tts", "Text is required for TTS", false);
  }

  try {
    const res = await fetch("https://api.openai.com/v1/audio/speech", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model: params.model,
        input: params.text,
        voice: params.voice,
        response_format: params.response_format,
        speed: params.speed,
      }),
    });

    if (!res.ok) {
      const errorText = await res.text();
      return bridgeError(
        "openai-tts",
        `OpenAI TTS HTTP ${res.status}: ${errorText.slice(0, 500)}`,
        res.status >= 500
      );
    }

    const audioBuffer = Buffer.from(await res.arrayBuffer());
    const ext = params.response_format === "wav" ? "wav" : "mp3";
    const outPath = path.join(
      args.output_dir,
      `openai-tts-${Date.now()}.${ext}`
    );
    fs.writeFileSync(outPath, audioBuffer);

    return {
      ok: true,
      provider: "openai",
      provider_job_id: `openai-tts-${Date.now()}`,
      status: "ready",
      artifacts: [
        {
          path: outPath,
          mime_type: `audio/${ext === "mp3" ? "mpeg" : ext}`,
          metadata: {
            model: params.model,
            voice: params.voice,
            text_length: params.text.length,
          },
        },
      ],
      metadata: {
        api: "POST /v1/audio/speech",
        model: params.model,
        params,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("openai-tts", message, true);
  }
}
