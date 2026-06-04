import fs from "node:fs";
import path from "node:path";
import { GoogleGenAI, Modality } from "@google/genai";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * Nano Banana 2 — gemini-3.1-flash-image-preview
 * @see https://ai.google.dev/gemini-api/docs/models/gemini-3.1-flash-image-preview
 */
export interface NanoBananaImageEditParams {
  api_model: string;
  aspect_ratio: string;
  image_size: string;
  person_generation: "dont_allow" | "allow_adult" | "allow_all";
  temperature: number;
  top_p: number;
  thinking_level: "minimal" | "low" | "medium" | "high";
  number_of_images: number;
  seed?: number;
  search_grounding: boolean;
}

const DEFAULT_MODEL = "gemini-3.1-flash-image-preview";

const DEFAULTS: NanoBananaImageEditParams = {
  api_model: DEFAULT_MODEL,
  aspect_ratio: "1:1",
  image_size: "1K",
  person_generation: "allow_adult",
  temperature: 0.8,
  top_p: 0.95,
  thinking_level: "medium",
  number_of_images: 1,
  search_grounding: false,
};

function parseParams(options: Record<string, unknown> | undefined): NanoBananaImageEditParams {
  const o = options ?? {};
  const cap =
    typeof o.google === "object" && o.google !== null
      ? (o.google as Record<string, unknown>)
      : o;

  const thinking = cap.thinking_level ?? cap.reasoning_effort;
  const imageSize = cap.image_size ?? cap.resolution;

  return {
    api_model: (cap.api_model as string) ?? DEFAULTS.api_model,
    aspect_ratio: (cap.aspect_ratio as string) ?? DEFAULTS.aspect_ratio,
    image_size: (imageSize as string) ?? DEFAULTS.image_size,
    person_generation:
      (cap.person_generation as NanoBananaImageEditParams["person_generation"]) ??
      DEFAULTS.person_generation,
    temperature:
      typeof cap.temperature === "number" ? cap.temperature : DEFAULTS.temperature,
    top_p: typeof cap.top_p === "number" ? cap.top_p : DEFAULTS.top_p,
    thinking_level:
      (thinking as NanoBananaImageEditParams["thinking_level"]) ?? DEFAULTS.thinking_level,
    number_of_images:
      typeof cap.number_of_images === "number"
        ? cap.number_of_images
        : DEFAULTS.number_of_images,
    seed: typeof cap.seed === "number" ? cap.seed : undefined,
    search_grounding:
      typeof cap.search_grounding === "boolean"
        ? cap.search_grounding
        : DEFAULTS.search_grounding,
  };
}

function readImagePart(filePath: string) {
  const data = fs.readFileSync(filePath).toString("base64");
  const ext = path.extname(filePath).toLowerCase();
  let mimeType = "image/png";
  if (ext === ".jpg" || ext === ".jpeg") mimeType = "image/jpeg";
  if (ext === ".webp") mimeType = "image/webp";
  return { inlineData: { mimeType, data } };
}

export async function runNanoBananaImageEdit(args: {
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
      "nano-banana",
      "Missing AETHER_GOOGLE_API_KEY, GEMINI_API_KEY, or GOOGLE_API_KEY",
      false
    );
  }

  if (args.input_image_paths.length === 0) {
    return bridgeError(
      "nano-banana",
      "image_edit requires at least one input image",
      false
    );
  }
  if (args.input_image_paths.length > 14) {
    return bridgeError(
      "nano-banana",
      "Nano Banana supports at most 14 reference images",
      false
    );
  }

  const params = parseParams(args.options);
  const ai = new GoogleGenAI({ apiKey });

  try {
    const parts = [
      { text: args.prompt },
      ...args.input_image_paths.map((p) => readImagePart(p)),
    ];

    const config: Record<string, unknown> = {
      responseModalities: [Modality.TEXT, Modality.IMAGE],
      temperature: params.temperature,
      topP: params.top_p,
      imageConfig: {
        aspectRatio: params.aspect_ratio,
        imageSize: params.image_size,
        personGeneration: params.person_generation,
      },
      thinkingConfig: { thinkingLevel: params.thinking_level },
    };

    if (params.seed !== undefined) {
      config.seed = params.seed;
    }

    if (params.search_grounding) {
      config.tools = [{ googleSearch: {} }];
    }

    const response = await ai.models.generateContent({
      model: params.api_model,
      contents: [{ role: "user", parts }],
      config: config as Parameters<typeof ai.models.generateContent>[0]["config"],
    });

    const artifacts: BridgeArtifact[] = [];
    for (const candidate of response.candidates ?? []) {
      for (const part of candidate.content?.parts ?? []) {
        if (part.inlineData?.data) {
          const mime = part.inlineData.mimeType ?? "image/png";
          const ext = mime.includes("jpeg") ? "jpg" : "png";
          const outPath = path.join(
            args.output_dir,
            `nano-banana-${Date.now()}-${artifacts.length}.${ext}`
          );
          fs.writeFileSync(outPath, Buffer.from(part.inlineData.data, "base64"));
          artifacts.push({
            path: outPath,
            mime_type: mime,
            metadata: {
              provider: "nano-banana",
              model: params.api_model,
              aspect_ratio: params.aspect_ratio,
              image_size: params.image_size,
              person_generation: params.person_generation,
              thinking_level: params.thinking_level,
              search_grounding: params.search_grounding,
            },
          });
          if (artifacts.length >= params.number_of_images) {
            break;
          }
        }
      }
      if (artifacts.length >= params.number_of_images) {
        break;
      }
    }

    if (artifacts.length === 0) {
      return bridgeError(
        "nano-banana",
        "Nano Banana returned no image in response",
        true
      );
    }

    return {
      ok: true,
      provider: "google",
      provider_job_id: `nano-banana-${Date.now()}`,
      status: "ready",
      artifacts,
      metadata: {
        api: "generateContent",
        display_name: "Nano Banana 2",
        model: params.api_model,
        params,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("nano-banana", message, true);
  }
}
