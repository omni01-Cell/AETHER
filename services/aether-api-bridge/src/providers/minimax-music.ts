import fs from "node:fs";
import path from "node:path";
import type { BridgeArtifact, BridgeFailure, BridgeSuccess } from "../protocol.js";
import { bridgeError } from "../protocol.js";

/**
 * MiniMax Music 2.5 — Music generation via FAL.AI
 * @see https://fal.ai/models/fal-ai/minimax-music
 */

export interface MiniMaxMusicParams {
  prompt: string;
  lyrics: string;
  duration: number;
  audio_setting: {
    sample_rate: number;
    bitrate: number;
    format: string;
  };
}

const DEFAULTS: MiniMaxMusicParams = {
  prompt: "",
  lyrics: "",
  duration: 60,
  audio_setting: {
    sample_rate: 32000,
    bitrate: 128000,
    format: "mp3",
  },
};

function parseParams(options: Record<string, unknown> | undefined): MiniMaxMusicParams {
  const o = options ?? {};
  const cap =
    typeof o.minimax === "object" && o.minimax !== null
      ? (o.minimax as Record<string, unknown>)
      : o;

  const audioSetRaw =
    typeof cap.audio_setting === "object" && cap.audio_setting !== null
      ? (cap.audio_setting as Record<string, unknown>)
      : {};

  const audio_setting = {
    sample_rate:
      typeof audioSetRaw.sample_rate === "number" && audioSetRaw.sample_rate > 0
        ? audioSetRaw.sample_rate
        : DEFAULTS.audio_setting.sample_rate,
    bitrate:
      typeof audioSetRaw.bitrate === "number" && audioSetRaw.bitrate > 0
        ? audioSetRaw.bitrate
        : DEFAULTS.audio_setting.bitrate,
    format:
      typeof audioSetRaw.format === "string"
        ? audioSetRaw.format
        : DEFAULTS.audio_setting.format,
  };

  return {
    prompt: typeof cap.prompt === "string" ? cap.prompt : DEFAULTS.prompt,
    lyrics: typeof cap.lyrics === "string" ? cap.lyrics : DEFAULTS.lyrics,
    duration: typeof cap.duration === "number" && cap.duration > 0 ? cap.duration : DEFAULTS.duration,
    audio_setting,
  };
}

export async function runMiniMaxMusic(args: {
  prompt: string;
  input_image_paths: string[];
  output_dir: string;
  options?: Record<string, unknown>;
}): Promise<BridgeSuccess | BridgeFailure> {
  const apiKey = process.env.FAL_API_KEY ?? process.env.MINIMAX_API_KEY;
  if (!apiKey?.trim()) {
    return bridgeError(
      "minimax-music",
      "Missing FAL_API_KEY or MINIMAX_API_KEY",
      false
    );
  }

  const params = parseParams(args.options);
  params.prompt = args.prompt.trim() || params.prompt;

  if (!params.prompt) {
    return bridgeError("minimax-music", "Prompt is required for music generation", false);
  }

  try {
    const submitRes = await fetch("https://fal.run/fal-ai/minimax-music/v2", {
      method: "POST",
      headers: {
        Authorization: `Key ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        prompt: params.prompt,
        lyrics: params.lyrics || undefined,
        duration: params.duration,
        audio_setting: params.audio_setting,
      }),
    });

    if (!submitRes.ok) {
      const errorText = await submitRes.text();
      return bridgeError(
        "minimax-music",
        `MiniMax Music HTTP ${submitRes.status}: ${errorText.slice(0, 500)}`,
        submitRes.status >= 500
      );
    }

    const submitData = (await submitRes.json()) as {
      request_id?: string;
      status?: string;
    };

    const requestId = submitData.request_id;
    if (!requestId) {
      return bridgeError("minimax-music", "No request_id returned", true);
    }

    let status = "submitted";
    let audioUrl: string | null = null;
    const maxAttempts = 60;
    let attempt = 0;

    while (attempt < maxAttempts) {
      await new Promise((resolve) => setTimeout(resolve, 3000));
      attempt++;

      const statusRes = await fetch(
        `https://fal.run/fal-ai/minimax-music/v2/${encodeURIComponent(requestId)}`,
        {
          headers: {
            Authorization: `Key ${apiKey}`,
          },
        }
      );

      if (!statusRes.ok) continue;

      const statusData = (await statusRes.json()) as {
        status?: string;
        audio_url?: string;
        output?: { audio_url?: string };
      };

      status = statusData.status ?? "processing";

      if (status === "COMPLETED" || status === "completed") {
        audioUrl = statusData.audio_url ?? statusData.output?.audio_url ?? null;
        break;
      }

      if (status === "FAILED" || status === "failed") {
        return bridgeError("minimax-music", "Music generation failed", true);
      }
    }

    if (!audioUrl) {
      return bridgeError("minimax-music", "Music generation timed out", true);
    }

    const audioRes = await fetch(audioUrl);
    if (!audioRes.ok) {
      return bridgeError(
        "minimax-music",
        `Failed to download audio: HTTP ${audioRes.status}`,
        true
      );
    }

    const audioBuffer = Buffer.from(await audioRes.arrayBuffer());
    const outPath = path.join(args.output_dir, `minimax-music-${Date.now()}.mp3`);
    fs.writeFileSync(outPath, audioBuffer);

    return {
      ok: true,
      provider: "minimax",
      provider_job_id: requestId,
      status: "ready",
      artifacts: [
        {
          path: outPath,
          mime_type: "audio/mpeg",
          metadata: {
            model: "minimax-music-v2",
            prompt: params.prompt,
            duration: params.duration,
          },
        },
      ],
      metadata: {
        api: "POST /fal-ai/minimax-music/v2",
        model: "minimax-music-v2",
        request_id: requestId,
        params,
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return bridgeError("minimax-music", message, true);
  }
}
