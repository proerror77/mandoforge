#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import process from "node:process";

const INDEX_FILE = process.env.INDEX_FILE ?? "web/index.html";
const API_FILE = process.env.API_FILE ?? "crates/mandoforge-api/src/http_shell.rs";
const CSP_VALUE = process.env.CSP_VALUE;

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

function scriptSource(policy) {
  return policy.match(/(?:^|;)\s*script-src\s+([^;]+)/i)?.[1] ?? null;
}

const expectedHash = inlineModuleScriptHash(read(INDEX_FILE));
if (CSP_VALUE) {
  const scriptSrc = scriptSource(CSP_VALUE);
  const actualHash = scriptSrc?.match(/sha256-[^'\s;]+/)?.[0] ?? null;
  if (actualHash !== expectedHash || scriptSrc.includes("'unsafe-inline'")) {
    console.error("live static UI CSP hash mismatch");
    console.error(`expected: ${expectedHash}`);
    console.error(`found: ${actualHash ?? "<missing>"}`);
    process.exit(1);
  }
  console.log(`live static UI CSP hash verification ok: ${expectedHash}`);
} else {
  const apiSource = read(API_FILE);
  if (
    !apiSource.includes('include_str!("../../../web/index.html")') ||
    !apiSource.includes("inline_module_script_hash(CONSOLE_INDEX_HTML)")
  ) {
    console.error("API CSP must derive from the embedded web/index.html bootstrap");
    process.exit(1);
  }
  console.log(`static UI CSP derives from embedded index: ${expectedHash}`);
}
