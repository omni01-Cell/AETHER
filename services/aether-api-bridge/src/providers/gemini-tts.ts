import fs from "node:fs";
import path from "node:path";
import { GoogleGenAI } from "@google/genai";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * Google Gemini TTS — Voice generation via Gemini API
 * @see https://ai.google.dev/gemini-api/docs/speech
 */

export interface GeminiTTSParams {
  text: string;
  voice_name: string;
  speaking_rate: number;
  pitch: number;
  volume_gain_db: number;
}

const DEFAULTS: GeminiTTSParams = {
  text: "",
  voice_name: "en-US-Neural2-F",
  speaking_rate: 1.0,
  pitch: 0.0,
  volume_gain_db: 0.0,
};

function parseParams(options: Record<string, unknown> | undefined): GeminiTTSParams {
  const o = options ?? {};
  const cap =
    typeof o.google === "object" && o.google !== null
      ? (o.google as Record<string, unknown>)
      : o;

  return {
    text: (cap.text as string) ?? DEFAULTS.text,
    voice_name: (cap.voice_name as string) ?? DEFAULTS.voice_name,
    speaking_rate:
      typeof cap.speaking_rate === "number"
        ? cap.speaking_rate
        : DEFAULTS.speaking_rate,
    pitch: typeof cap.pitch === "number" ? cap.pitch : DEFAULTS.pitch,
    volume_gain_db:
      typeof cap.volume_gain_db === "number"
        ? cap.volume_gain_db
        : DEFAULTS.volume_gain_db,
  };
}

export async function runGeminiTTS(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey =
    process.env.AETHER_GOOGLE_API_KEY ??
    process.env.GEMINI_API_KEY ??
    process.env.GOOGLE_API_KEY;

  if (!apiKey) {
    return bridgeError(
      "gemini-tts",
      "Missing AETHER_GOOGLE_API_KEY, GEMINI_API_KEY, or GOOGLE_API_KEY",
      false
    );
  }

  const params = parseParams(args.options);
  params.text = args.prompt || params.text;

  if (!params.text) {
    return bridgeError("gemini-tts", "Text is required for TTS", false);
  }

  try {
    const ai = new GoogleGenAI({ apiKey });

    const response = await ai.models.generateContent({
      model: "gemini-3.1-flash-tts",
      contents: [{ role: "user", parts: [{ text: params.text }] }],
      config: {
        responseModalities: ["AUDIO"],
        speechConfig: {
          voiceConfig: {
            prebuiltVoiceConfig: {
              voiceName: params.voice_name,
            },
          },
        },
      } as Parameters<typeof ai.models.generateContent>[0]["config"],
    });

    // Extract audio from response
    for (const candidate of response.candidates ?? []) {
      for (const part of candidate.content?.parts ?? []) {
        if (part.inlineData?.data) {
          const mime = part.inlineData.mimeType ?? "audio/wav";
          const ext = mime.includes("wav") ? "wav" : mime.includes("mp3") ? "mp3" : "pcm";
          const outPath = path.join(
            args.output_dir,
            `gemini-tts-${Date.now()}.${ext}`
          );
          fs.writeFileSync(outPath, Buffer.from(part.inlineData.data, "base64"));

          return {
            ok: true,
            provider: "google",
            provider_job_id: `gemini-tts-${Date.now()}`,
            status: "ready",
            artifacts: [
              {
                path: outPath,
                mime_type: mime,
                metadata: {
                  model: "gemini-3.1-flash-tts",
                  voice: params.voice_name,
                  text_length: params.text.length,
                },
              },
            ],
            metadata: {
              api: "generateContent",
              model: "gemini-3.1-flash-tts",
              params,
            },
          };
        }
      }
    }

    return bridgeError("gemini-tts", "Gemini TTS returned no audio", true);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("gemini-tts", message, true);
  }
}