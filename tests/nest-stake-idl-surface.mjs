import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const idl = JSON.parse(
  await readFile(new URL("../target/idl/nest_stake.json", import.meta.url), "utf8"),
);
const instructions = new Map(idl.instructions.map((instruction) => [instruction.name, instruction]));

for (const removed of [
  "request_unstake",
  "complete_unstake",
  "cancel_expired_unstake",
  "migrate_legacy_pending_unstake",
  "migrate_pending_withdrawal",
]) {
  assert.equal(instructions.has(removed), false, `${removed} must not remain callable`);
}

const unstake = instructions.get("unstake");
assert.ok(unstake, "instant unstake instruction is required");
assert.deepEqual(
  unstake.args,
  [
    { name: "shares", type: "u64" },
    { name: "min_nusd_out", type: "u64" },
  ],
  "unstake must bind both the requested shares and minimum nUSD output",
);
assert.deepEqual(
  unstake.accounts.map((account) => account.name),
  [
    "staking_state",
    "protocol",
    "nest_core_program",
    "nusd_mint",
    "snusd_mint",
    "owner_snusd_account",
    "staking_nusd_vault",
    "owner_nusd_account",
    "owner",
    "nusd_token_program",
    "snusd_token_program",
  ],
);
assert.equal(unstake.accounts.find((account) => account.name === "owner")?.signer, true);

const stakingState = idl.types.find((type) => type.name === "StakingState");
assert.ok(stakingState, "StakingState layout is required");
assert.deepEqual(
  stakingState.type.fields,
  [
    { name: "authority", type: "pubkey" },
    { name: "nusd_mint", type: "pubkey" },
    { name: "snusd_mint", type: "pubkey" },
    { name: "staking_nusd_vault", type: "pubkey" },
    { name: "revenue_nusd_vault", type: "pubkey" },
    { name: "revenue_baseline_nusd", type: "u128" },
    { name: "total_shares", type: "u128" },
    { name: "total_user_shares", type: "u128" },
    { name: "staking_vault_nusd", type: "u128" },
    { name: "realized_loss_nusd", type: "u128" },
    { name: "unvested_revenue", type: "u128" },
    { name: "reserved_pending_claims", type: "u128" },
    { name: "vesting_start_ts", type: "i64" },
    { name: "vesting_end_ts", type: "i64" },
    { name: "last_vesting_sync_ts", type: "i64" },
    { name: "cooldown_seconds", type: "i64" },
    { name: "revenue_vesting_seconds", type: "i64" },
    { name: "paused", type: "bool" },
    { name: "bump", type: "u8" },
  ],
  "the deployed staking account layout must not change",
);

const errors = new Map(idl.errors.map((error) => [error.name, error.code]));
assert.equal(errors.get("BadDebtOutstanding"), 6007);
assert.equal(errors.get("StakeCapacityExceeded"), 6013);

console.log("nest-stake IDL surface checks passed");
