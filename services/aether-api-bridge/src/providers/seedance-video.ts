import fs from "node:fs";
import path from "node:path";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * Seedance 2.0 — ByteDance video generation
 * @see https://seed.bytedance.com/en/seedance2_0
 * @see https://docs.byteplus.com/en/docs/ModelArk/1520757
 */

export interface SeedanceVideoParams {
  prompt: string;
  aspect_ratio: string;
  duration: number;
  resolution: "480p" | "720p";
  model: "fast" | "standard";
  generate_audio: boolean;
  seed: number;
  watermark: boolean;
  first_frame?: string;
  last_frame?: string;
  reference_images?: string[];
  reference_videos?: string[];
  reference_audios?: string[];
}

const DEFAULTS: SeedanceVideoParams = {
  prompt: "",
  aspect_ratio: "16:9",
  duration: 5,
  resolution: "720p",
  model: "standard",
  generate_audio: true,
  seed: 42,
  watermark: false,
};

function parseParams(
  options: Record<string, unknown> | undefined,
  prompt: string,
  inputImagePaths: string[]
): SeedanceVideoParams {
  const o = options ?? {};
  const cap =
    typeof o.bytedance === "object" && o.bytedance !== null
      ? (o.bytedance as Record<string, unknown>)
      : o;

  const validRes: Set<string> = new Set(["480p", "720p"]);
  const validModels: Set<string> = new Set(["fast", "standard"]);

  const resStr = typeof cap.resolution === "string" ? cap.resolution : "";
  const modelStr = typeof cap.model === "string" ? cap.model : "";

  const params: SeedanceVideoParams = {
    prompt,
    aspect_ratio: typeof cap.aspect_ratio === "string" ? cap.aspect_ratio : DEFAULTS.aspect_ratio,
    duration: typeof cap.duration === "number" ? cap.duration : DEFAULTS.duration,
    resolution: validRes.has(resStr)
      ? (resStr as SeedanceVideoParams["resolution"])
      : DEFAULTS.resolution,
    model: validModels.has(modelStr)
      ? (modelStr as SeedanceVideoParams["model"])
      : DEFAULTS.model,
    generate_audio:
      typeof cap.generate_audio === "boolean"
        ? cap.generate_audio
        : DEFAULTS.generate_audio,
    seed: typeof cap.seed === "number" ? cap.seed : DEFAULTS.seed,
    watermark:
      typeof cap.watermark === "boolean" ? cap.watermark : DEFAULTS.watermark,
  };

  // Image-to-video: first frame from input images
  if (inputImagePaths.length > 0) {
    params.first_frame = inputImagePaths[0];
    if (inputImagePaths.length > 1) {
      params.last_frame = inputImagePaths[1];
    }
  }

  // Reference images for multimodal mode
  if (inputImagePaths.length > 2) {
    params.reference_images = inputImagePaths.slice(2);
  }

  return params;
}

async function readImageAsBase64(filePath: string): Promise<{ mimeType: string; data: string }> {
  const buffer = await fs.promises.readFile(filePath);
  const data = buffer.toString("base64");
  const ext = path.extname(filePath).toLowerCase();
  let mimeType = "image/png";
  if (ext === ".jpg" || ext === ".jpeg") mimeType = "image/jpeg";
  if (ext === ".webp") mimeType = "image/webp";
  return { mimeType, data };
}

export async function runSeedanceVideo(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey =
    process.env.AETHER_BYTEDANCE_API_KEY ??
    process.env.SEEDANCE_API_KEY ??
    process.env.VOLCENGINE_API_KEY;

  if (!apiKey) {
    return bridgeError(
      "seedance",
      "Missing AETHER_BYTEDANCE_API_KEY, SEEDANCE_API_KEY, or VOLCENGINE_API_KEY",
      false
    );
  }

  const params = parseParams(args.options, args.prompt, args.input_image_paths);

  try {
    const body: Record<string, unknown> = {
      prompt: params.prompt,
      aspect_ratio: params.aspect_ratio,
      duration: params.duration,
      resolution: params.resolution,
      model: params.model === "fast" ? "seedance-2.0-fast" : "seedance-2.0",
      generate_audio: params.generate_audio,
      seed: params.seed,
      watermark: params.watermark,
    };

    if (params.first_frame) {
      const img = await readImageAsBase64(params.first_frame);
      body.first_frame = `data:${img.mimeType};base64,${img.data}`;
    }

    if (params.last_frame) {
      const img = await readImageAsBase64(params.last_frame);
      body.last_frame = `data:${img.mimeType};base64,${img.data}`;
    }

    if (params.reference_images && params.reference_images.length > 0) {
      const refs = await Promise.all(
        params.reference_images.map(async (p) => {
          const img = await readImageAsBase64(p);
          return `data:${img.mimeType};base64,${img.data}`;
        })
      );
      body.reference_images = refs;
    }

    const submitRes = await fetch("https://api.segmind.com/v1/seedance-2.0", {
      method: "POST",
      headers: {
        "x-api-key": apiKey,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });

    if (!submitRes.ok) {
      const errorText = await submitRes.text();
      return bridgeError(
        "seedance",
        `Seedance API HTTP ${submitRes.status}: ${errorText.slice(0, 500)}`,
        submitRes.status >= 500
      );
    }

    const videoBuffer = Buffer.from(await submitRes.arrayBuffer());
    const outPath = path.join(
      args.output_dir,
      `seedance-${Date.now()}.mp4`
    );
    await fs.promises.writeFile(outPath, videoBuffer);

    return {
      ok: true,
      provider: "bytedance",
      provider_job_id: `seedance-${Date.now()}`,
      status: "ready",
      artifacts: [
        {
          path: outPath,
          mime_type: "video/mp4",
          metadata: {
            model: params.model,
            aspect_ratio: params.aspect_ratio,
            duration: params.duration,
            resolution: params.resolution,
            generate_audio: params.generate_audio,
          },
        },
      ],
      metadata: {
        api: "POST /v1/seedance-2.0",
        model: params.model,
        params: { ...params },
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("seedance", message, true);
  }
}
