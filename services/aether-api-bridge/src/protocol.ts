/** JSON IPC protocol between AETHER (Rust) and this bridge (stdin/stdout). */

export const BRIDGE_VERSION = 1;

export type BridgeOperation = "image_edit" | "chat_completions" | "video_generate" | "voice_generate" | "music_generate";

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
