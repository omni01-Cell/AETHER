import fs from "node:fs";
import path from "node:path";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * Kling 3.0 — Kuaishou video generation
 * @see https://kling.ai/quickstart/klingai-video-3-model-user-guide
 */

export interface KlingVideoParams {
  prompt: string;
  aspect_ratio: string;
  duration: number;
  mode: "std" | "pro" | "4k";
  generate_audio: boolean;
  cfg_scale: number;
  seed: number;
  multi_shots: boolean;
  first_frame?: string;
  last_frame?: string;
}

const DEFAULTS: KlingVideoParams = {
  prompt: "",
  aspect_ratio: "16:9",
  duration: 5,
  mode: "std",
  generate_audio: true,
  cfg_scale: 0.5,
  seed: -1,
  multi_shots: false,
};

function parseParams(
  options: Record<string, unknown> | undefined,
  prompt: string,
  inputImagePaths: string[]
): KlingVideoParams {
  const o = options ?? {};
  const cap =
    typeof o.kuaishou === "object" && o.kuaishou !== null
      ? (o.kuaishou as Record<string, unknown>)
      : o;

  const params: KlingVideoParams = {
    prompt,
    aspect_ratio: (cap.aspect_ratio as string) ?? DEFAULTS.aspect_ratio,
    duration: typeof cap.duration === "number" ? cap.duration : DEFAULTS.duration,
    mode: (cap.mode as KlingVideoParams["mode"]) ?? DEFAULTS.mode,
    generate_audio:
      typeof cap.generate_audio === "boolean"
        ? cap.generate_audio
        : DEFAULTS.generate_audio,
    cfg_scale: typeof cap.cfg_scale === "number" ? cap.cfg_scale : DEFAULTS.cfg_scale,
    seed: typeof cap.seed === "number" ? cap.seed : DEFAULTS.seed,
    multi_shots:
      typeof cap.multi_shots === "boolean" ? cap.multi_shots : DEFAULTS.multi_shots,
  };

  // Image-to-video: first frame from input images
  if (inputImagePaths.length > 0) {
    params.first_frame = inputImagePaths[0];
    if (inputImagePaths.length > 1) {
      params.last_frame = inputImagePaths[1];
    }
  }

  return params;
}

function readImageAsBase64(filePath: string): { mimeType: string; data: string } {
  const data = fs.readFileSync(filePath).toString("base64");
  const ext = path.extname(filePath).toLowerCase();
  let mimeType = "image/png";
  if (ext === ".jpg" || ext === ".jpeg") mimeType = "image/jpeg";
  if (ext === ".webp") mimeType = "image/webp";
  return { mimeType, data };
}

export async function runKlingVideo(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey =
    process.env.AETHER_KUAISHOU_API_KEY ??
    process.env.KLING_API_KEY;

  if (!apiKey) {
    return bridgeError(
      "kling",
      "Missing AETHER_KUAISHOU_API_KEY or KLING_API_KEY",
      false
    );
  }

  const params = parseParams(args.options, args.prompt, args.input_image_paths);

  try {
    // Build request body for Kling API
    const body: Record<string, unknown> = {
      model: `kling-v3-${params.mode}`,
      input: {
        prompt: params.prompt,
        aspect_ratio: params.aspect_ratio,
        duration: params.duration,
        generate_audio: params.generate_audio,
        cfg_scale: params.cfg_scale,
        seed: params.seed,
        multi_shots: params.multi_shots,
        negative_prompt: "blur, distort, and low quality",
      },
    };

    // Image-to-video mode
    if (params.first_frame) {
      const img = readImageAsBase64(params.first_frame);
      (body.input as Record<string, unknown>).image_urls = [
        `data:${img.mimeType};base64,${img.data}`,
      ];

      if (params.last_frame) {
        const lastImg = readImageAsBase64(params.last_frame);
        ((body.input as Record<string, unknown>).image_urls as string[]).push(
          `data:${lastImg.mimeType};base64,${lastImg.data}`
        );
      }
    }

    // Submit generation task
    const submitRes = await fetch("https://api.klingai.com/v1/videos/generations", {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });

    if (!submitRes.ok) {
      const errorText = await submitRes.text();
      return bridgeError(
        "kling",
        `Kling API HTTP ${submitRes.status}: ${errorText.slice(0, 500)}`,
        submitRes.status >= 500
      );
    }

    const submitData = (await submitRes.json()) as {
      code: number;
      data?: { task_id: string };
      message?: string;
    };

    if (submitData.code !== 200 || !submitData.data?.task_id) {
      return bridgeError(
        "kling",
        `Kling API error: ${submitData.message ?? "Unknown error"}`,
        false
      );
    }

    const taskId = submitData.data.task_id;

    // Poll for completion
    let status = "submitted";
    let videoUrl: string | null = null;
    const maxAttempts = 120; // 10 minutes max (5s intervals)
    let attempt = 0;

    while (attempt < maxAttempts) {
      await new Promise((resolve) => setTimeout(resolve, 5000));
      attempt++;

      const statusRes = await fetch(
        `https://api.klingai.com/v1/videos/generations/${taskId}`,
        {
          headers: {
            Authorization: `Bearer ${apiKey}`,
          },
        }
      );

      if (!statusRes.ok) {
        continue;
      }

      const statusData = (await statusRes.json()) as {
        code: number;
        data?: {
          status: string;
          task_result?: { videos?: Array<{ url: string }> };
        };
      };

      if (statusData.code !== 200 || !statusData.data) {
        continue;
      }

      status = statusData.data.status;

      if (status === "succeed" && statusData.data.task_result?.videos?.[0]) {
        videoUrl = statusData.data.task_result.videos[0].url;
        break;
      }

      if (status === "failed") {
        return bridgeError("kling", "Kling video generation failed", true);
      }
    }

    if (!videoUrl) {
      return bridgeError(
        "kling",
        "Kling video generation timed out",
        true
      );
    }

    // Download the video
    const videoRes = await fetch(videoUrl);
    if (!videoRes.ok) {
      return bridgeError(
        "kling",
        `Failed to download video: HTTP ${videoRes.status}`,
        true
      );
    }

    const videoBuffer = Buffer.from(await videoRes.arrayBuffer());
    const outPath = path.join(args.output_dir, `kling-${Date.now()}.mp4`);
    fs.writeFileSync(outPath, videoBuffer);

    return {
      ok: true,
      provider: "kuaishou",
      provider_job_id: taskId,
      status: "ready",
      artifacts: [
        {
          path: outPath,
          mime_type: "video/mp4",
          metadata: {
            model: params.mode,
            aspect_ratio: params.aspect_ratio,
            duration: params.duration,
            generate_audio: params.generate_audio,
          },
        },
      ],
      metadata: {
        api: "POST /v1/videos/generations",
        model: params.mode,
        task_id: taskId,
        params,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("kling", message, true);
  }
}