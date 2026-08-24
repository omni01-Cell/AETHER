#!/usr/bin/env node
import fs from "node:fs";
import type { BridgeResponse } from "./protocol.js";
import { BRIDGE_VERSION, bridgeError, isBridgeRequest, sanitizePath } from "./protocol.js";
import { dispatch } from "./router.js";

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    if (Buffer.isBuffer(chunk)) {
      chunks.push(chunk);
    } else {
      chunks.push(Buffer.from(chunk as string | Uint8Array));
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
    writeResponse(
      bridgeError(
        "bridge",
        "Invalid BridgeRequest schema: missing or invalid required fields",
        false
      )
    );
    process.exit(1);
    return;
  }

  const req = parsed;

  if (req.version !== BRIDGE_VERSION) {
    writeResponse(
      bridgeError("bridge", `Unsupported bridge version: ${req.version}`, false)
    );
    process.exit(1);
    return;
  }

  // Sanitize paths in the request
  if (req.output_dir) {
    try {
      req.output_dir = sanitizePath(req.output_dir);
    } catch (err) {
      writeResponse(
        bridgeError(
          "bridge",
          `Invalid output_dir path: ${err instanceof Error ? err.message : String(err)}`,
          false
        )
      );
      process.exit(1);
      return;
    }
  }

  if (req.input_image_paths) {
    try {
      req.input_image_paths = req.input_image_paths.map((p) => sanitizePath(p));
    } catch (err) {
      writeResponse(
        bridgeError(
          "bridge",
          `Invalid input_image_paths: ${err instanceof Error ? err.message : String(err)}`,
          false
        )
      );
      process.exit(1);
      return;
    }
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
            `Cannot create output_dir: ${req.output_dir} (${e instanceof Error ? e.message : String(e)})`,
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
