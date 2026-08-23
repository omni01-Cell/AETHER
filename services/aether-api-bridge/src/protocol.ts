/** JSON IPC protocol between AETHER (Rust) and this bridge (stdin/stdout). */

export const BRIDGE_VERSION = 1;

export type BridgeOperation =
  | "image_edit"
  | "chat_completions"
  | "video_generate"
  | "voice_generate"
  | "music_generate";

export type GenerationStatusBridge =
  | "queued"
  | "submitted"
  | "running"
  | "ready"
  | "failed"
  | "cancelled";

export interface BridgeArtifact {
  path: string;
  mime_type: string;
  metadata?: Record<string, unknown>;
}

export interface BridgeChatMessage {
  role: string;
  content: string;
}

export interface BridgeRequest {
  version: number;
  operation: BridgeOperation;
  /** AETHER model id, e.g. openai/gpt-image-2 or openai/gpt-4o-mini */
  model_id: string;
  /** TS handler id from agents.v1.json or routing.v1.json */
  bridge_handler?: string;
  /** Agent name for chat_completions, e.g. prompter, planner */
  agent?: string;
  provider?: string;
  api_model?: string;
  /** image_edit */
  prompt?: string;
  input_image_paths?: string[];
  output_dir?: string;
  /** chat_completions */
  messages?: BridgeChatMessage[];
  tools?: unknown[];
  options?: Record<string, unknown>;
}

export interface BridgeSuccess {
  ok: true;
  provider: string;
  provider_job_id: string;
  status: GenerationStatusBridge;
  artifacts: BridgeArtifact[];
  metadata?: Record<string, unknown>;
}

export interface BridgeFailure {
  ok: false;
  provider: string;
  error: string;
  retryable: boolean;
  metadata?: Record<string, unknown>;
}

export type BridgeResponse = BridgeSuccess | BridgeFailure;

export function bridgeError(
  provider: string,
  message: string,
  retryable = true
): BridgeFailure {
  return { ok: false, provider, error: message, retryable };
}

const VALID_OPERATIONS: ReadonlySet<string> = new Set([
  "image_edit",
  "chat_completions",
  "video_generate",
  "voice_generate",
  "music_generate",
]);

/** Strict runtime validator for incoming BridgeRequest IPC messages. */
export function isBridgeRequest(val: unknown): val is BridgeRequest {
  if (typeof val !== "object" || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  if (typeof obj.version !== "number" || !Number.isInteger(obj.version)) {
    return false;
  }
  if (typeof obj.operation !== "string" || !VALID_OPERATIONS.has(obj.operation)) {
    return false;
  }
  if (typeof obj.model_id !== "string" || !obj.model_id.trim()) {
    return false;
  }
  if (obj.bridge_handler !== undefined && typeof obj.bridge_handler !== "string") {
    return false;
  }
  if (obj.agent !== undefined && typeof obj.agent !== "string") {
    return false;
  }
  if (obj.provider !== undefined && typeof obj.provider !== "string") {
    return false;
  }
  if (obj.api_model !== undefined && typeof obj.api_model !== "string") {
    return false;
  }
  if (obj.prompt !== undefined && typeof obj.prompt !== "string") {
    return false;
  }
  if (
    obj.input_image_paths !== undefined &&
    (!Array.isArray(obj.input_image_paths) ||
      !obj.input_image_paths.every((p) => typeof p === "string"))
  ) {
    return false;
  }
  if (obj.output_dir !== undefined && typeof obj.output_dir !== "string") {
    return false;
  }
  if (
    obj.messages !== undefined &&
    (!Array.isArray(obj.messages) ||
      !obj.messages.every(
        (m) =>
          typeof m === "object" &&
          m !== null &&
          typeof (m as Record<string, unknown>).role === "string" &&
          typeof (m as Record<string, unknown>).content === "string"
      ))
  ) {
    return false;
  }
  if (
    obj.options !== undefined &&
    (typeof obj.options !== "object" || obj.options === null)
  ) {
    return false;
  }
  return true;
}
