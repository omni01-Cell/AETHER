import assert from "node:assert/strict";
import { test } from "node:test";
import { BRIDGE_VERSION, bridgeError, isBridgeRequest } from "../protocol.js";
import { dispatch } from "../router.js";

test("isBridgeRequest validates valid requests", () => {
  const req = {
    version: BRIDGE_VERSION,
    operation: "image_edit",
    model_id: "openai/gpt-image-2",
    bridge_handler: "openai-image-edit",
    prompt: "A beautiful sunset",
    input_image_paths: ["/tmp/test.png"],
    output_dir: "/tmp/out",
  };

  assert.equal(isBridgeRequest(req), true);
});

test("isBridgeRequest rejects invalid requests", () => {
  assert.equal(isBridgeRequest(null), false);
  assert.equal(isBridgeRequest({}), false);
  assert.equal(isBridgeRequest({ version: "1", operation: "image_edit", model_id: "m" }), false);
  assert.equal(isBridgeRequest({ version: 1, operation: "unknown_op", model_id: "m" }), false);
  assert.equal(isBridgeRequest({ version: 1, operation: "image_edit", model_id: "" }), false);
  assert.equal(
    isBridgeRequest({
      version: 1,
      operation: "image_edit",
      model_id: "m",
      input_image_paths: [123],
    }),
    false
  );
});

test("bridgeError generates expected failure object", () => {
  const err = bridgeError("openai", "Test error", false);
  assert.deepEqual(err, {
    ok: false,
    provider: "openai",
    error: "Test error",
    retryable: false,
  });
});

test("dispatch returns error for unknown operation or missing handler", async () => {
  const req = {
    version: BRIDGE_VERSION,
    operation: "image_edit" as const,
    model_id: "test-model",
    bridge_handler: "non-existent-handler",
    output_dir: "/tmp",
  };

  const res = await dispatch(req);
  assert.equal(res.ok, false);
  if (!res.ok) {
    assert.equal(res.provider, "bridge");
    assert.match(res.error, /image_edit requires bridge_handler/);
  }
});
