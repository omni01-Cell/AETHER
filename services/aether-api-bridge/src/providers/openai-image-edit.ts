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

interface OpenAiImagesEditResponse {
  data?: Array<{ b64_json?: string; revised_prompt?: string }>;
  usage?: Record<string, unknown>;
  error?: { message?: string };
}

function isOpenAiImagesEditResponse(val: unknown): val is OpenAiImagesEditResponse {
  if (typeof val !== "object" || val === null) return false;
  const obj = val as Record<string, unknown>;
  if (obj.data !== undefined && !Array.isArray(obj.data)) return false;
  return true;
}

function parseParams(options: Record<string, unknown> | undefined): OpenAiImageEditParams {
  const o = options ?? {};
  const cap =
    typeof o.openai === "object" && o.openai !== null
      ? (o.openai as Record<string, unknown>)
      : o;

  const validQualities: Set<string> = new Set(["low", "medium", "high", "auto"]);
  const validSizes: Set<string> = new Set(["auto", "1024x1024", "1536x1024", "1024x1536"]);
  const validFormats: Set<string> = new Set(["png", "jpeg", "webp"]);
  const validBackgrounds: Set<string> = new Set(["transparent", "opaque", "auto"]);
  const validFidelities: Set<string> = new Set(["high", "low"]);
  const validModerations: Set<string> = new Set(["low", "auto"]);

  const qualityStr = typeof cap.quality === "string" ? cap.quality : "";
  const sizeStr = typeof cap.size === "string" ? cap.size : "";
  const formatStr = typeof cap.output_format === "string" ? cap.output_format : "";
  const bgStr = typeof cap.background === "string" ? cap.background : "";
  const fidelityStr = typeof cap.input_fidelity === "string" ? cap.input_fidelity : "";
  const modStr = typeof cap.moderation === "string" ? cap.moderation : "";

  return {
    api_model: typeof cap.api_model === "string" ? cap.api_model : DEFAULTS.api_model,
    quality: validQualities.has(qualityStr)
      ? (qualityStr as OpenAiImageEditParams["quality"])
      : DEFAULTS.quality,
    size: validSizes.has(sizeStr)
      ? (sizeStr as OpenAiImageEditParams["size"])
      : DEFAULTS.size,
    n: typeof cap.n === "number" ? cap.n : DEFAULTS.n,
    output_format: validFormats.has(formatStr)
      ? (formatStr as OpenAiImageEditParams["output_format"])
      : DEFAULTS.output_format,
    output_compression:
      typeof cap.output_compression === "number"
        ? cap.output_compression
        : undefined,
    background: validBackgrounds.has(bgStr)
      ? (bgStr as OpenAiImageEditParams["background"])
      : DEFAULTS.background,
    input_fidelity: validFidelities.has(fidelityStr)
      ? (fidelityStr as OpenAiImageEditParams["input_fidelity"])
      : DEFAULTS.input_fidelity,
    moderation: validModerations.has(modStr)
      ? (modStr as OpenAiImageEditParams["moderation"])
      : DEFAULTS.moderation,
    mask_path: typeof cap.mask_path === "string" ? cap.mask_path : undefined,
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
      const buffer = await fs.promises.readFile(p);
      const file = new File([buffer], path.basename(p), {
        type: mimeFromPath(p),
      });
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
      const maskBuffer = await fs.promises.readFile(params.mask_path);
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

    const rawJson: unknown = await httpRes.json();
    const response: OpenAiImagesEditResponse = isOpenAiImagesEditResponse(rawJson)
      ? rawJson
      : {};

    if (!httpRes.ok) {
      const msg =
        response.error?.message ??
        (typeof rawJson === "object" && rawJson !== null
          ? JSON.stringify(rawJson)
          : httpRes.statusText);
      return bridgeError("openai", `OpenAI HTTP ${httpRes.status}: ${msg}`, httpRes.status >= 500);
    }

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
      await fs.promises.writeFile(outPath, Buffer.from(b64, "base64"));
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
