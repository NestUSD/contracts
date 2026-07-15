import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const idl = JSON.parse(
  readFileSync(new URL("../target/idl/nest_core.json", import.meta.url), "utf8"),
);

const instruction = idl.instructions.find((item) => item.name === "deposit_protocol_revenue");
assert.ok(instruction, "deposit_protocol_revenue instruction missing from core IDL");
assert.deepEqual(
  instruction.accounts.map((account) => account.name),
  [
    "protocol",
    "staking_state",
    "nusd_mint",
    "source_nusd_account",
    "insurance_nusd_vault",
    "staker_revenue_nusd_vault",
    "protocol_revenue_nusd_vault",
    "source",
    "nusd_token_program",
  ],
  "deposit_protocol_revenue account order changed",
);
assert.deepEqual(instruction.args, [{ name: "amount", type: "u64" }]);

const event = idl.events.find((item) => item.name === "ProtocolRevenueDeposited");
assert.ok(event, "ProtocolRevenueDeposited event missing from core IDL");
const eventType = idl.types.find((item) => item.name === event.name);
assert.ok(eventType, "ProtocolRevenueDeposited event type missing from core IDL");
assert.deepEqual(
  eventType.type.fields.map((field) => field.name),
  [
    "protocol",
    "source",
    "source_nusd_account",
    "amount_nusd",
    "bad_debt_repaid_nusd",
    "insurance_nusd",
    "staker_nusd",
    "protocol_nusd",
  ],
  "ProtocolRevenueDeposited event fields changed",
);

console.log("nest_core revenue IDL surface checks passed");
