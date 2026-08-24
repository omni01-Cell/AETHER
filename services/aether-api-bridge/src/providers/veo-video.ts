import fs from "node:fs";
import path from "node:path";
import { GoogleGenAI } from "@google/genai";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * Veo 3.1 — Google video generation
 * @see https://ai.google.dev/gemini-api/docs/models/veo
 */

export interface VeoVideoParams {
  prompt: string;
  aspect_ratio: string;
  duration_sec: number;
  negative_prompt?: string;
}

const DEFAULTS: VeoVideoParams = {
  prompt: "",
  aspect_ratio: "16:9",
  duration_sec: 8,
};

function parseParams(
  options: Record<string, unknown> | undefined,
  prompt: string
): VeoVideoParams {
  const o = options ?? {};
  const cap =
    typeof o.google === "object" && o.google !== null
      ? (o.google as Record<string, unknown>)
      : o;

  return {
    prompt,
    aspect_ratio: typeof cap.aspect_ratio === "string" ? cap.aspect_ratio : DEFAULTS.aspect_ratio,
    duration_sec:
      typeof cap.duration_sec === "number"
        ? cap.duration_sec
        : DEFAULTS.duration_sec,
    negative_prompt:
      typeof cap.negative_prompt === "string"
        ? cap.negative_prompt
        : undefined,
  };
}

async function readImagePart(p: string) {
  const buffer = await fs.promises.readFile(p);
  const data = buffer.toString("base64");
  const ext = path.extname(p).toLowerCase();
  let mimeType = "image/png";
  if (ext === ".jpg" || ext === ".jpeg") mimeType = "image/jpeg";
  if (ext === ".webp") mimeType = "image/webp";
  return { inlineData: { mimeType, data } };
}

export async function runVeoVideo(args: {
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
      "veo",
      "Missing AETHER_GOOGLE_API_KEY, GEMINI_API_KEY, or GOOGLE_API_KEY",
      false
    );
  }

  const params = parseParams(args.options, args.prompt);

  try {
    const ai = new GoogleGenAI({ apiKey });

    // Build config for Veo
    const config: Record<string, unknown> = {
      aspectRatio: params.aspect_ratio,
      durationSec: params.duration_sec,
    };

    if (params.negative_prompt) {
      config.negativePrompt = params.negative_prompt;
    }

    // If we have input images, use image-to-video
    let parts: Array<{ inlineData: { mimeType: string; data: string } } | { text: string }>;
    if (args.input_image_paths.length > 0) {
      const imageParts = await Promise.all(
        args.input_image_paths.map((p) => readImagePart(p))
      );
      parts = [...imageParts, { text: params.prompt }];
    } else {
      parts = [{ text: params.prompt }];
    }

    const response = await ai.models.generateContent({
      model: "veo-3.1",
      contents: [{ role: "user", parts }],
      config: config as Parameters<typeof ai.models.generateContent>[0]["config"],
    });

    // Extract video from response
    const artifacts: BridgeArtifact[] = [];
    for (const candidate of response.candidates ?? []) {
      for (const part of candidate.content?.parts ?? []) {
        if (part.inlineData?.data) {
          const mime = part.inlineData.mimeType ?? "video/mp4";
          const ext = mime.includes("webm") ? "webm" : "mp4";
          const outPath = path.join(
            args.output_dir,
            `veo-${Date.now()}-${artifacts.length}.${ext}`
          );
          await fs.promises.writeFile(outPath, Buffer.from(part.inlineData.data, "base64"));
          artifacts.push({
            path: outPath,
            mime_type: mime,
            metadata: {
              provider: "google",
              model: "veo-3.1",
              aspect_ratio: params.aspect_ratio,
              duration_sec: params.duration_sec,
            },
          });
        }
      }
    }

    if (artifacts.length === 0) {
      return bridgeError("veo", "Veo returned no video in response", true);
    }

    return {
      ok: true,
      provider: "google",
      provider_job_id: `veo-${Date.now()}`,
      status: "ready",
      artifacts,
      metadata: {
        api: "generateContent",
        model: "veo-3.1",
        params: { ...params },
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("veo", message, true);
  }
}
