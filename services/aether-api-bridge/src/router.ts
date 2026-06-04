import type { BridgeFailure, BridgeRequest, BridgeResponse } from "./protocol.js";
import { bridgeError } from "./protocol.js";
import { runNanoBananaImageEdit } from "./providers/nano-banana-image-edit.js";
import { runOpenAiChat } from "./providers/openai-chat.js";
import { runOpenAiImageEdit } from "./providers/openai-image-edit.js";
import { runSeedanceVideo } from "./providers/seedance-video.js";
import { runKlingVideo } from "./providers/kling-video.js";
import { runVeoVideo } from "./providers/veo-video.js";
import { runElevenLabsTTS } from "./providers/elevenlabs-tts.js";
import { runGeminiTTS } from "./providers/gemini-tts.js";
import { runOpenAITTS } from "./providers/openai-tts.js";
import { runMiniMaxMusic } from "./providers/minimax-music.js";

const IMAGE_HANDLERS: Record<
  string,
  (req: BridgeRequest) => Promise<BridgeResponse>
> = {
  "openai-image-edit": (req) =>
    runOpenAiImageEdit({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
  "nano-banana-image-edit": (req) =>
    runNanoBananaImageEdit({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
};

const VIDEO_HANDLERS: Record<
  string,
  (req: BridgeRequest) => Promise<BridgeResponse>
> = {
  "seedance-video": (req) =>
    runSeedanceVideo({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
  "kling-video": (req) =>
    runKlingVideo({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
  "veo-video": (req) =>
    runVeoVideo({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
};

const VOICE_HANDLERS: Record<
  string,
  (req: BridgeRequest) => Promise<BridgeResponse>
> = {
  "elevenlabs-tts": (req) =>
    runElevenLabsTTS({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
  "gemini-tts": (req) =>
    runGeminiTTS({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
  "openai-tts": (req) =>
    runOpenAITTS({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
};

const MUSIC_HANDLERS: Record<
  string,
  (req: BridgeRequest) => Promise<BridgeResponse>
> = {
  "minimax-music": (req) =>
    runMiniMaxMusic({
      prompt: req.prompt ?? "",
      input_image_paths: req.input_image_paths ?? [],
      output_dir: req.output_dir ?? "",
      options: req.options,
    }),
};

const CHAT_HANDLERS: Record<
  string,
  (req: BridgeRequest) => Promise<BridgeResponse>
> = {
  "openai-chat": runOpenAiChat,
};

export async function dispatch(req: BridgeRequest): Promise<BridgeResponse> {
  const handler = req.bridge_handler?.trim();

  if (req.operation === "chat_completions") {
    const run =
      (handler && CHAT_HANDLERS[handler]) ?? CHAT_HANDLERS["openai-chat"];
    if (!run) {
      return bridgeError(
        "bridge",
        `Unknown bridge_handler for chat: ${handler}`,
        false
      );
    }
    return run(req);
  }

  if (req.operation === "image_edit") {
    if (handler && IMAGE_HANDLERS[handler]) {
      return IMAGE_HANDLERS[handler](req);
    }
    return bridgeError(
      "bridge",
      `image_edit requires bridge_handler (got: ${handler ?? "none"})`,
      false
    );
  }

  if (req.operation === "video_generate") {
    if (handler && VIDEO_HANDLERS[handler]) {
      return VIDEO_HANDLERS[handler](req);
    }
    return bridgeError(
      "bridge",
      `video_generate requires bridge_handler (got: ${handler ?? "none"})`,
      false
    );
  }

  if (req.operation === "voice_generate") {
    if (handler && VOICE_HANDLERS[handler]) {
      return VOICE_HANDLERS[handler](req);
    }
    return bridgeError(
      "bridge",
      `voice_generate requires bridge_handler (got: ${handler ?? "none"})`,
      false
    );
  }

  if (req.operation === "music_generate") {
    if (handler && MUSIC_HANDLERS[handler]) {
      return MUSIC_HANDLERS[handler](req);
    }
    return bridgeError(
      "bridge",
      `music_generate requires bridge_handler (got: ${handler ?? "none"})`,
      false
    );
  }

  return bridgeError("bridge", `Unsupported operation: ${req.operation}`, false);
}