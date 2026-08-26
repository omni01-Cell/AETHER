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

  const aspect_ratio = typeof cap.aspect_ratio === "string" ? cap.aspect_ratio : DEFAULTS.aspect_ratio;
  const duration_sec = typeof cap.duration_sec === "number" ? cap.duration_sec : DEFAULTS.duration_sec;
  const negative_prompt = typeof cap.negative_prompt === "string" ? cap.negative_prompt : undefined;

  return {
    prompt,
    aspect_ratio,
    duration_sec,
    negative_prompt,
  };
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

  for (const p of args.input_image_paths) {
    if (!fs.existsSync(p)) {
      return bridgeError("veo", `Input image file not found: ${p}`, false);
    }
  }

  const params = parseParams(args.options, args.prompt);

  try {
    const ai = new GoogleGenAI({ apiKey });

    let imageParam: { imageBytes: string; mimeType: string } | undefined;
    if (args.input_image_paths.length > 0) {
      const firstPath = args.input_image_paths[0];
      const data = fs.readFileSync(firstPath).toString("base64");
      const ext = path.extname(firstPath).toLowerCase();
      let mimeType = "image/png";
      if (ext === ".jpg" || ext === ".jpeg") mimeType = "image/jpeg";
      if (ext === ".webp") mimeType = "image/webp";
      imageParam = { imageBytes: data, mimeType };
    }

    const operation = await ai.models.generateVideos({
      model: "veo-3.1",
      prompt: params.prompt,
      ...(imageParam ? { image: imageParam } : {}),
      config: {
        aspectRatio: params.aspect_ratio,
        ...(params.negative_prompt ? { negativePrompt: params.negative_prompt } : {}),
      },
    });

    const response = operation.response;
    const generatedVideos = response?.generatedVideos ?? [];
    const artifacts: BridgeArtifact[] = [];

    for (let i = 0; i < generatedVideos.length; i++) {
      const vid = generatedVideos[i];
      const videoBytes = vid.video?.videoBytes;
      if (videoBytes) {
        const ext = "mp4";
        const outPath = path.join(
          args.output_dir,
          `veo-${Date.now()}-${i}.${ext}`
        );
        fs.writeFileSync(outPath, Buffer.from(videoBytes, "base64"));
        artifacts.push({
          path: outPath,
          mime_type: "video/mp4",
          metadata: {
            provider: "google",
            model: "veo-3.1",
            aspect_ratio: params.aspect_ratio,
            duration_sec: params.duration_sec,
          },
        });
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
        api: "generateVideos",
        model: "veo-3.1",
        params,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("veo", message, true);
  }
}
