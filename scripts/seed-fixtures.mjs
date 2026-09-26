#!/usr/bin/env node
// Seed fixtures/ark-snapshot.json into a running Kosmos Engine.
//
// Usage:
//   node scripts/seed-fixtures.mjs [--data-dir <dir>] [--snapshot <file>]
//
// Engine discovery: <data-dir>/engine.lock.json (KOSMOS_DATA_DIR env or
// --data-dir; default ~/.config/Kosmos). Speaks Engine `/v1/rpc` as a
// desktop-host client — the same path Host E2E helpers use — so no app grant
// is required and all object types in the snapshot are writable.
import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { randomUUID } from "node:crypto";

const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const dataDir =
  opt("--data-dir") ||
  process.env.KOSMOS_DATA_DIR ||
  path.join(process.env.XDG_CONFIG_HOME || `${process.env.HOME}/.config`, "Kosmos");
const snapshotPath =
  opt("--snapshot") ||
  path.join(path.dirname(new URL(import.meta.url).pathname), "..", "fixtures", "ark-snapshot.json");

const lock = JSON.parse(readFileSync(path.join(dataDir, "engine.lock.json"), "utf8"));
if (!lock.http_port || !lock.auth_token) {
  console.error(`engine.lock.json in ${dataDir} is missing http_port/auth_token`);
  process.exit(2);
}

async function rpc(operation, params = {}) {
  const res = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "desktop-host",
      "X-Kosmos-Client-Version": "0.1.0",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
  });
  return res.json();
}

const snapshot = JSON.parse(readFileSync(snapshotPath, "utf8"));
let ok = 0;
const failed = [];

for (const object of snapshot.objects) {
  if (object.$seed === false) continue; // reader-path fixtures: canonical ingress rejects them
  const r = await rpc("upsert_object", { object });
  if (r.ok === true) ok += 1;
  else failed.push({ id: object.id, error: r.error ?? r });
}

for (const link of snapshot.links) {
  const r = await rpc("upsert_object_link", { object_link: link });
  if (r.ok === true) ok += 1;
  else failed.push({ id: link.id, error: r.error ?? r });
}

console.log(`seeded ${ok}/${snapshot.objects.length + snapshot.links.length} records`);
if (failed.length) {
  console.error("failed:", JSON.stringify(failed, null, 2));
  process.exit(1);
}
