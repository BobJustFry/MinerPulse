import assert from "node:assert/strict";
import test from "node:test";
import { decryptPassword, encryptPassword, normalizeMac } from "./miner-credentials-crypto.js";

test("normalizeMac uppercases and uses colons", () => {
  assert.equal(normalizeMac("ca-01-14-00-04-eb"), "CA:01:14:00:04:EB");
});

test("encryptPassword round-trips", () => {
  process.env.MINER_CREDENTIALS_KEY = Buffer.alloc(32, 7).toString("base64");
  const plain = "miner-secret-42";
  const enc = encryptPassword(plain);
  assert.notEqual(enc, plain);
  assert.equal(decryptPassword(enc), plain);
});
