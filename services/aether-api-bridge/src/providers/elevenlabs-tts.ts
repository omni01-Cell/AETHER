import fs from "node:fs";
import path from "node:path";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * ElevenLabs TTS — High-quality voice generation
 * @see https://elevenlabs.io/docs/api-reference/text-to-speech
 */

export interface ElevenLabsTTSParams {
  text: string;
  model_id: string;
  voice_id: string;
  stability: number;
  similarity_boost: number;
  style: number;
  output_format: string;
}

const DEFAULTS: ElevenLabsTTSParams = {
  text: "",
  model_id: "eleven_v3",
  voice_id: "21m00Tcm4TlvDq8ikWAM",
  stability: 0.5,
  similarity_boost: 0.75,
  style: 0,
  output_format: "mp3_44100_192",
};

function parseParams(options: Record<string, unknown> | undefined): ElevenLabsTTSParams {
  const o = options ?? {};
  const cap =
    typeof o.elevenlabs === "object" && o.elevenlabs !== null
      ? (o.elevenlabs as Record<string, unknown>)
      : o;

  const text = typeof cap.text === "string" ? cap.text : DEFAULTS.text;
  const model_id = typeof cap.model_id === "string" ? cap.model_id : DEFAULTS.model_id;
  const voice_id = typeof cap.voice_id === "string" ? cap.voice_id : DEFAULTS.voice_id;
  const stability = typeof cap.stability === "number" ? cap.stability : DEFAULTS.stability;
  const similarity_boost = typeof cap.similarity_boost === "number" ? cap.similarity_boost : DEFAULTS.similarity_boost;
  const style = typeof cap.style === "number" ? cap.style : DEFAULTS.style;
  const output_format = typeof cap.output_format === "string" ? cap.output_format : DEFAULTS.output_format;

  return {
    text,
    model_id,
    voice_id,
    stability,
    similarity_boost,
    style,
    output_format,
  };
}

export async function runElevenLabsTTS(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey = process.env.ELEVENLABS_API_KEY;
  if (!apiKey) {
    return bridgeError(
      "elevenlabs",
      "Missing ELEVENLABS_API_KEY",
      false
    );
  }

  const params = parseParams(args.options);
  params.text = args.prompt || params.text;

  if (!params.text) {
    return bridgeError("elevenlabs", "Text is required for TTS", false);
  }

  try {
    const url = `https://api.elevenlabs.io/v1/text-to-speech/${params.voice_id}`;
    const body = {
      text: params.text,
      model_id: params.model_id,
      voice_settings: {
        stability: params.stability,
        similarity_boost: params.similarity_boost,
        style: params.style,
      },
    };

    const res = await fetch(url, {
      method: "POST",
      headers: {
        "xi-api-key": apiKey,
        "Content-Type": "application/json",
        Accept: "audio/mpeg",
      },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const errorText = await res.text();
      return bridgeError(
        "elevenlabs",
        `ElevenLabs HTTP ${res.status}: ${errorText.slice(0, 500)}`,
        res.status >= 500
      );
    }

    const audioBuffer = Buffer.from(await res.arrayBuffer());
    const ext = params.output_format.startsWith("mp3")
      ? "mp3"
      : params.output_format.startsWith("wav")
        ? "wav"
        : "pcm";
    const outPath = path.join(
      args.output_dir,
      `elevenlabs-${Date.now()}.${ext}`
    );
    fs.writeFileSync(outPath, audioBuffer);

    return {
      ok: true,
      provider: "elevenlabs",
      provider_job_id: `elevenlabs-${Date.now()}`,
      status: "ready",
      artifacts: [
        {
          path: outPath,
          mime_type: `audio/${ext === "mp3" ? "mpeg" : ext}`,
          metadata: {
            model: params.model_id,
            voice_id: params.voice_id,
            text_length: params.text.length,
          },
        },
      ],
      metadata: {
        api: "POST /v1/text-to-speech/{voice_id}",
        model: params.model_id,
        params,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("elevenlabs", message, true);
  }
}
