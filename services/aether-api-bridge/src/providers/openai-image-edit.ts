import fs from "node:fs";
import path from "node:path";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/** Params from https://developers.openai.com/api/reference/resources/images/methods/edit */
export interface OpenAiImageEditParams {
  api_model: string;
  quality: "low" | "medium" | "high" | "auto";
  size: "auto" | "1024x1024" | "1536x1024" | "1024x1536";
  n: number;
  output_format: "png" | "jpeg" | "webp";
  output_compression?: number;
  background: "transparent" | "opaque" | "auto";
  input_fidelity: "high" | "low";
  moderation: "low" | "auto";
  mask_path?: string;
}

const DEFAULTS: OpenAiImageEditParams = {
  api_model: "gpt-image-2",
  quality: "medium",
  size: "auto",
  n: 1,
  output_format: "png",
  background: "auto",
  input_fidelity: "low",
  moderation: "auto",
};

const VALID_QUALITIES = new Set<OpenAiImageEditParams["quality"]>(["low", "medium", "high", "auto"]);
const VALID_SIZES = new Set<OpenAiImageEditParams["size"]>(["auto", "1024x1024", "1536x1024", "1024x1536"]);
const VALID_FORMATS = new Set<OpenAiImageEditParams["output_format"]>(["png", "jpeg", "webp"]);
const VALID_BACKGROUNDS = new Set<OpenAiImageEditParams["background"]>(["transparent", "opaque", "auto"]);
const VALID_FIDELITIES = new Set<OpenAiImageEditParams["input_fidelity"]>(["high", "low"]);
const VALID_MODERATIONS = new Set<OpenAiImageEditParams["moderation"]>(["low", "auto"]);

function parseParams(options: Record<string, unknown> | undefined): OpenAiImageEditParams {
  const o = options ?? {};
  const cap =
    typeof o.openai === "object" && o.openai !== null
      ? (o.openai as Record<string, unknown>)
      : o;

  const api_model = typeof cap.api_model === "string" ? cap.api_model : DEFAULTS.api_model;
  const quality = typeof cap.quality === "string" && VALID_QUALITIES.has(cap.quality as OpenAiImageEditParams["quality"])
    ? (cap.quality as OpenAiImageEditParams["quality"])
    : DEFAULTS.quality;
  const size = typeof cap.size === "string" && VALID_SIZES.has(cap.size as OpenAiImageEditParams["size"])
    ? (cap.size as OpenAiImageEditParams["size"])
    : DEFAULTS.size;
  const n = typeof cap.n === "number" && cap.n > 0 ? cap.n : DEFAULTS.n;
  const output_format = typeof cap.output_format === "string" && VALID_FORMATS.has(cap.output_format as OpenAiImageEditParams["output_format"])
    ? (cap.output_format as OpenAiImageEditParams["output_format"])
    : DEFAULTS.output_format;
  const output_compression = typeof cap.output_compression === "number" ? cap.output_compression : undefined;
  const background = typeof cap.background === "string" && VALID_BACKGROUNDS.has(cap.background as OpenAiImageEditParams["background"])
    ? (cap.background as OpenAiImageEditParams["background"])
    : DEFAULTS.background;
  const input_fidelity = typeof cap.input_fidelity === "string" && VALID_FIDELITIES.has(cap.input_fidelity as OpenAiImageEditParams["input_fidelity"])
    ? (cap.input_fidelity as OpenAiImageEditParams["input_fidelity"])
    : DEFAULTS.input_fidelity;
  const moderation = typeof cap.moderation === "string" && VALID_MODERATIONS.has(cap.moderation as OpenAiImageEditParams["moderation"])
    ? (cap.moderation as OpenAiImageEditParams["moderation"])
    : DEFAULTS.moderation;
  const mask_path = typeof cap.mask_path === "string" ? cap.mask_path : undefined;

  return {
    api_model,
    quality,
    size,
    n,
    output_format,
    output_compression,
    background,
    input_fidelity,
    moderation,
    mask_path,
  };
}

export async function runOpenAiImageEdit(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey = process.env.AETHER_OPENAI_API_KEY ?? process.env.OPENAI_API_KEY;
  if (!apiKey) {
    return bridgeError(
      "openai",
      "Missing AETHER_OPENAI_API_KEY or OPENAI_API_KEY",
      false
    );
  }

  if (args.input_image_paths.length === 0) {
    return bridgeError("openai", "image_edit requires at least one input image", false);
  }
  if (args.input_image_paths.length > 16) {
    return bridgeError("openai", "OpenAI image edit supports at most 16 input images", false);
  }

  const params = parseParams(args.options);

  try {
    const form = new FormData();
    form.append("model", params.api_model);
    form.append("prompt", args.prompt);

    for (const p of args.input_image_paths) {
      if (!fs.existsSync(p)) {
        return bridgeError("openai", `Input image file not found: ${p}`, false);
      }
      const buffer = fs.readFileSync(p);
      const file = new File([buffer], path.basename(p), { type: mimeFromPath(p) });
      form.append("image[]", file);
    }

    form.append("quality", params.quality);
    form.append("size", params.size);
    form.append("n", String(params.n));
    form.append("output_format", params.output_format);
    form.append("background", params.background);
    form.append("input_fidelity", params.input_fidelity);
    form.append("moderation", params.moderation);
    if (params.output_compression !== undefined) {
      form.append("output_compression", String(params.output_compression));
    }
    if (params.mask_path) {
      if (!fs.existsSync(params.mask_path)) {
        return bridgeError("openai", `Mask file not found: ${params.mask_path}`, false);
      }
      const maskBuffer = fs.readFileSync(params.mask_path);
      const maskFile = new File([maskBuffer], path.basename(params.mask_path), {
        type: mimeFromPath(params.mask_path),
      });
      form.append("mask", maskFile);
    }

    const httpRes = await fetch("https://api.openai.com/v1/images/edits", {
      method: "POST",
      headers: { Authorization: `Bearer ${apiKey}` },
      body: form,
    });

    const responseText = await httpRes.text();
    let responseObj: unknown;
    try {
      responseObj = JSON.parse(responseText);
    } catch {
      responseObj = null;
    }

    if (!httpRes.ok) {
      const msg =
        typeof responseObj === "object" && responseObj !== null && "error" in responseObj
          ? JSON.stringify((responseObj as { error: unknown }).error)
          : responseText.slice(0, 500) || httpRes.statusText;
      return bridgeError("openai", `OpenAI HTTP ${httpRes.status}: ${msg}`, httpRes.status >= 500);
    }

    const response = isOpenAiImagesEditResponse(responseObj) ? responseObj : {};
    const artifacts: BridgeArtifact[] = [];
    const ext = params.output_format === "jpeg" ? "jpg" : params.output_format;

    const dataList = response.data ?? [];
    for (let i = 0; i < dataList.length; i++) {
      const item = dataList[i];
      const b64 = item.b64_json;
      if (!b64) {
        return bridgeError("openai", "OpenAI response missing b64_json for edited image", true);
      }
      const outPath = path.join(
        args.output_dir,
        `openai-edit-${Date.now()}-${i}.${ext}`
      );
      fs.writeFileSync(outPath, Buffer.from(b64, "base64"));
      artifacts.push({
        path: outPath,
        mime_type: mimeFromFormat(params.output_format),
        metadata: {
          revised_prompt: item.revised_prompt,
          model: params.api_model,
          quality: params.quality,
        },
      });
    }

    if (artifacts.length === 0) {
      return bridgeError("openai", "OpenAI returned no image data", true);
    }

    return {
      ok: true,
      provider: "openai",
      provider_job_id: `openai-edit-${Date.now()}`,
      status: "ready",
      artifacts,
      metadata: {
        api: "POST /v1/images/edits",
        model: params.api_model,
        quality: params.quality,
        usage: response.usage,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    const retryable = !message.includes("invalid_api_key");
    return bridgeError("openai", message, retryable);
  }
}

function mimeFromPath(p: string): string {
  const ext = path.extname(p).toLowerCase();
  if (ext === ".png") return "image/png";
  if (ext === ".jpg" || ext === ".jpeg") return "image/jpeg";
  if (ext === ".webp") return "image/webp";
  return "application/octet-stream";
}

function mimeFromFormat(fmt: string): string {
  if (fmt === "jpeg") return "image/jpeg";
  if (fmt === "webp") return "image/webp";
  return "image/png";
}

interface OpenAiImagesEditResponse {
  data?: Array<{ b64_json?: string; revised_prompt?: string }>;
  usage?: Record<string, unknown>;
  error?: { message?: string };
}

function isOpenAiImagesEditResponse(obj: unknown): obj is OpenAiImagesEditResponse {
  return typeof obj === "object" && obj !== null;
}
