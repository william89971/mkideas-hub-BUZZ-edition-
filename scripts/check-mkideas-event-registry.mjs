import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const registry = JSON.parse(
  readFileSync(join(root, "docs/nips/mkideas-event-registry.json"), "utf8"),
);
const rust = readFileSync(join(root, "crates/buzz-core/src/kind.rs"), "utf8");
const typescript = readFileSync(
  join(root, "desktop/src/shared/constants/kinds.ts"),
  "utf8",
);
const dart = readFileSync(
  join(root, "mobile/lib/shared/relay/nostr_models.dart"),
  "utf8",
);

const constants = {
  30800: ["KIND_MK_GOAL", "mkGoal"],
  30801: ["KIND_MK_OPERATIONAL_PROJECT", "mkOperationalProject"],
  30802: ["KIND_MK_TASK", "mkTask"],
  30803: ["KIND_MK_PERSON", "mkPerson"],
  30804: ["KIND_MK_INTERVIEW", "mkInterview"],
  30805: ["KIND_MK_CONTENT", "mkContent"],
  30806: ["KIND_MK_MEETING", "mkMeeting"],
  30807: ["KIND_MK_DECISION", "mkDecision"],
  30808: ["KIND_MK_KNOWLEDGE", "mkKnowledge"],
  30809: ["KIND_MK_APPROVAL", "mkApproval"],
  48200: ["KIND_MK_APPROVAL_ACTION", "mkApprovalAction"],
  48201: ["KIND_MK_AGENT_PROPOSAL", "mkAgentProposal"],
  48202: ["KIND_MK_MIGRATION_RECEIPT", "mkMigrationReceipt"],
  48203: ["KIND_MK_GENERATED_SUMMARY", "mkGeneratedSummary"],
  48204: ["KIND_MK_SYSTEM_ACTIVITY", "mkSystemActivity"],
  48205: ["KIND_MK_EXTERNAL_COMMUNICATION", "mkExternalCommunication"],
};

const registered = new Set([
  ...Object.keys(registry.state_kinds),
  ...Object.keys(registry.operation_kinds),
].map(Number));

for (const [kindText, [sharedName, dartName]] of Object.entries(constants)) {
  const kind = Number(kindText);
  if (!registered.has(kind)) {
    throw new Error(`kind ${kind} is absent from the NIP-MK registry`);
  }
  const rustPattern = new RegExp(`pub const ${sharedName}: u32 = ${kind};`);
  const tsPattern = new RegExp(`export const ${sharedName} = ${kind};`);
  const dartPattern = new RegExp(`static const ${dartName} = ${kind};`);
  if (!rustPattern.test(rust)) throw new Error(`Rust mirror drift: ${sharedName}`);
  if (!tsPattern.test(typescript)) throw new Error(`TypeScript mirror drift: ${sharedName}`);
  if (!dartPattern.test(dart)) throw new Error(`Dart mirror drift: ${dartName}`);
}

if (registered.size !== Object.keys(constants).length) {
  throw new Error("registry contains allocated kinds without language mirrors");
}

console.log(`NIP-MK registry synchronized across ${registered.size} allocated kinds.`);
