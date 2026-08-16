import { MandoForgeOntologySdk } from "./generated/fixture.js";

declare const process: {
  env: Record<string, string | undefined>;
};

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.trim() === "") {
    throw new Error(`missing required environment variable ${name}`);
  }
  return value;
}

function headersFromEnv(): Record<string, string> {
  const raw = process.env.MANDOFORGE_HEADERS_JSON;
  if (raw === undefined || raw.trim() === "") return {};
  const parsed: unknown = JSON.parse(raw);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("MANDOFORGE_HEADERS_JSON must be a JSON object");
  }
  const headers: Record<string, string> = {};
  for (const [name, value] of Object.entries(parsed)) {
    if (typeof value !== "string") throw new Error(`header ${name} must be a string`);
    headers[name] = value;
  }
  return headers;
}

export async function runConsumer(): Promise<void> {
  const sdk = new MandoForgeOntologySdk({
    baseUrl: requiredEnv("MANDOFORGE_BASE_URL"),
    headers: headersFromEnv(),
  });
  const taskGrantId = requiredEnv("MANDOFORGE_TASK_GRANT_ID");
  const sessionId = requiredEnv("MANDOFORGE_SESSION_ID");
  const contextPacketId = requiredEnv("MANDOFORGE_CONTEXT_PACKET_ID");
  const inventoryItemId = requiredEnv("MANDOFORGE_INVENTORY_ITEM_ID");

  const tickets = await sdk.objects.SupportTicket.list({ taskGrantId });
  const relations = await sdk.relations.customerCreatesSupportTicket.list({ taskGrantId });
  const proposal = await sdk.actions.adjustInventory.propose({
    sessionId,
    taskGrantId,
    contextPacketId,
    parameters: { inventory_item_id: inventoryItemId, delta_quantity: 1, reason: "consumer example" },
  });
  console.log(JSON.stringify({ ticketCount: tickets.length, relationCount: relations.length, proposalStatus: proposal.status }));
}

if (process.env.MANDOFORGE_RUN_LIVE === "1") {
  await runConsumer();
}
