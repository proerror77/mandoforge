#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import process from "node:process";

const INDEX_FILE = process.env.INDEX_FILE ?? "web/index.html";
const API_FILE = process.env.API_FILE ?? "crates/mandoforge-api/src/main.rs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function inlineModuleScriptHash(indexHtml) {
  const match = indexHtml.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!match) {
    throw new Error(`${INDEX_FILE} does not contain a Trunk inline module script`);
  }
  const digest = crypto.createHash("sha256").update(match[1]).digest("base64");
  return `sha256-${digest}`;
}

function cspHashes(apiSource) {
  return [
    ...apiSource.matchAll(
      /CONSOLE(?:_DEV)?_CONTENT_SECURITY_POLICY: &str = "([^"]+)"/g,
    ),
  ].map((match) => {
    const hash = match[1].match(/sha256-[^'\s;]+/);
    return hash?.[0] ?? null;
  });
}

const expectedHash = inlineModuleScriptHash(read(INDEX_FILE));
const hashes = cspHashes(read(API_FILE));

if (hashes.length !== 2 || hashes.some((hash) => hash !== expectedHash)) {
  console.error("static UI CSP hash mismatch");
  console.error(`expected: ${expectedHash}`);
  console.error(`found: ${hashes.map((hash) => hash ?? "<missing>").join(", ")}`);
  process.exit(1);
}

console.log(`static UI CSP hash verification ok: ${expectedHash}`);
