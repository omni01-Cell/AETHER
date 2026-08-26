#!/usr/bin/env node
import fs from "node:fs";
import type { BridgeOperation, BridgeRequest, BridgeResponse } from "./protocol.js";
import { BRIDGE_VERSION, bridgeError } from "./protocol.js";
import { dispatch } from "./router.js";

const VALID_OPERATIONS = new Set<BridgeOperation>([
  "image_edit",
  "chat_completions",
  "video_generate",
  "voice_generate",
  "music_generate",
]);

function isBridgeRequest(obj: unknown): obj is BridgeRequest {
  if (typeof obj !== "object" || obj === null) {
    return false;
  }
  const r = obj as Record<string, unknown>;

  if (typeof r.version !== "number") {
    return false;
  }
  if (typeof r.operation !== "string" || !VALID_OPERATIONS.has(r.operation as BridgeOperation)) {
    return false;
  }
  if (typeof r.model_id !== "string") {
    return false;
  }
  if (r.bridge_handler !== undefined && typeof r.bridge_handler !== "string") {
    return false;
  }
  if (r.agent !== undefined && typeof r.agent !== "string") {
    return false;
  }
  if (r.provider !== undefined && typeof r.provider !== "string") {
    return false;
  }
  if (r.api_model !== undefined && typeof r.api_model !== "string") {
    return false;
  }
  if (r.prompt !== undefined && typeof r.prompt !== "string") {
    return false;
  }
  if (r.input_image_paths !== undefined) {
    if (!Array.isArray(r.input_image_paths) || !r.input_image_paths.every((p) => typeof p === "string")) {
      return false;
    }
  }
  if (r.output_dir !== undefined && typeof r.output_dir !== "string") {
    return false;
  }
  if (r.messages !== undefined) {
    if (!Array.isArray(r.messages)) {
      return false;
    }
    for (const msg of r.messages) {
      if (typeof msg !== "object" || msg === null) return false;
      const m = msg as Record<string, unknown>;
      if (typeof m.role !== "string" || typeof m.content !== "string") return false;
    }
  }
  if (r.tools !== undefined && !Array.isArray(r.tools)) {
    return false;
  }
  if (r.options !== undefined && (typeof r.options !== "object" || r.options === null)) {
    return false;
  }

  return true;
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    if (Buffer.isBuffer(chunk)) {
      chunks.push(chunk);
    } else {
      chunks.push(Buffer.from(String(chunk), "utf8"));
    }
  }
  return Buffer.concat(chunks).toString("utf8");
}

function writeResponse(res: BridgeResponse): void {
  process.stdout.write(JSON.stringify(res));
}

async function main(): Promise<void> {
  const raw = await readStdin();
  if (!raw.trim()) {
    writeResponse(bridgeError("bridge", "Empty stdin (expected JSON BridgeRequest)", false));
    process.exit(1);
    return;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    writeResponse(bridgeError("bridge", "Invalid JSON on stdin", false));
    process.exit(1);
    return;
  }

  if (!isBridgeRequest(parsed)) {
    writeResponse(bridgeError("bridge", "Invalid BridgeRequest schema on stdin", false));
    process.exit(1);
    return;
  }

  const req: BridgeRequest = parsed;

  if (req.version !== BRIDGE_VERSION) {
    writeResponse(
      bridgeError("bridge", `Unsupported bridge version: ${req.version}`, false)
    );
    process.exit(1);
    return;
  }

  if (req.operation === "image_edit") {
    if (!req.output_dir) {
      writeResponse(
        bridgeError("bridge", "image_edit requires output_dir", false)
      );
      process.exit(1);
      return;
    }
    if (!fs.existsSync(req.output_dir)) {
      try {
        fs.mkdirSync(req.output_dir, { recursive: true });
      } catch (e) {
        writeResponse(
          bridgeError(
            "bridge",
            `Cannot create output_dir: ${req.output_dir} (${e})`,
            false
          )
        );
        process.exit(1);
        return;
      }
    }
  }

  const res = await dispatch(req);
  writeResponse(res);
  process.exit(res.ok ? 0 : 1);
}

main().catch((err) => {
  writeResponse(
    bridgeError("bridge", err instanceof Error ? err.message : String(err), false)
  );
  process.exit(1);
});
