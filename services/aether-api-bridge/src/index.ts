#!/usr/bin/env node
import fs from "node:fs";
import type { BridgeRequest, BridgeResponse } from "./protocol.js";
import { BRIDGE_VERSION, bridgeError } from "./protocol.js";
import { dispatch } from "./router.js";

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk as Buffer);
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

  let req: BridgeRequest;
  try {
    req = JSON.parse(raw) as BridgeRequest;
  } catch {
    writeResponse(bridgeError("bridge", "Invalid JSON on stdin", false));
    process.exit(1);
    return;
  }

  if (req.version !== BRIDGE_VERSION) {
    writeResponse(
      bridgeError("bridge", `Unsupported bridge version: ${req.version}`, false)
    );
    process.exit(1);
    return;
  }

  if (req.operation !== "chat_completions") {
    if (!req.output_dir) {
      writeResponse(
        bridgeError("bridge", `${req.operation} requires output_dir`, false)
      );
      process.exit(1);
      return;
    }
  }

  if (req.output_dir && !fs.existsSync(req.output_dir)) {
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
