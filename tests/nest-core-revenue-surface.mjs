import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const sourceFiles = [
  "programs/nest-core/src/helpers/token.rs",
  "programs/nest-core/src/ix/cdp.rs",
  "programs/nest-core/src/ix/revenue.rs",
  "programs/nest-core/src/ix/kamino.rs",
  "programs/nest-core/src/ix/liquidation/instant.rs",
  "programs/nest-core/src/ix/liquidation/two_step.rs",
];

const sources = new Map(
  sourceFiles.map((path) => [
    path,
    readFileSync(new URL(`../${path}`, import.meta.url), "utf8"),
  ]),
);
const combinedSource = [...sources.values()].join("\n");

assert.ok(
  !combinedSource.includes("require_recorded_staker_revenue_covered"),
  "core must not treat cumulative staker revenue as an outstanding backing liability",
);

for (const [path, source] of sources) {
  if (!source.includes("staker_revenue_nusd_vault")) continue;
  assert.ok(
    source.includes("require_token_account_increase"),
    `${path} must guard staker revenue vault token deltas`,
  );
}

assertStakerDeltaGuard("programs/nest-core/src/ix/cdp.rs", "staker_delta_u64");
assertStakerDeltaGuard("programs/nest-core/src/ix/revenue.rs", "staker_delta_u64");
assertStakerDeltaGuard("programs/nest-core/src/ix/kamino.rs", "staker_delta_u64");
assertStakerDeltaGuard("programs/nest-core/src/ix/liquidation/instant.rs", "staker_delta_u64");
assertStakerDeltaGuard("programs/nest-core/src/ix/liquidation/two_step.rs", "staker_delta_u64");
assertStakerDeltaGuard("programs/nest-core/src/ix/revenue.rs", "stakers");

const revenueInstruction = sources.get("programs/nest-core/src/ix/revenue.rs");
assert.ok(revenueInstruction, "missing core revenue instruction source");
assert.match(
  revenueInstruction,
  /pub fn deposit_protocol_revenue[\s\S]*?require_token_account_decrease\([\s\S]*?source_before,[\s\S]*?source_nusd_account\.amount,[\s\S]*?amount,[\s\S]*?\)\?/,
  "external revenue must exact-check the full source nUSD decrease",
);
assert.match(
  revenueInstruction,
  /require_mint_supply_decrease\([\s\S]*?nusd_supply_before,[\s\S]*?nusd_mint\.supply,[\s\S]*?bad_debt_repaid,[\s\S]*?\)\?/,
  "external revenue must exact-check nUSD burned against bad debt",
);
for (const [amountName, vaultName] of [
  ["insurance", "insurance_nusd_vault"],
  ["protocol_revenue", "protocol_revenue_nusd_vault"],
]) {
  assert.match(
    revenueInstruction,
    new RegExp([
      `if\\s+${amountName}\\s*>\\s*0\\s*\\{`,
      `let\\s+${amountName === "insurance" ? "insurance" : "protocol_revenue"}_before`,
      `${vaultName}\\.to_account_info\\(\\)`,
      `${vaultName}\\.reload\\(\\)\\?`,
      "require_token_account_increase\\(",
      `${amountName === "insurance" ? "insurance" : "protocol_revenue"}_before,`,
      `ctx\\.accounts\\.${vaultName}\\.amount,`,
      `${amountName},`,
    ].join("[\\s\\S]*?")),
    `external revenue must exact-check ${amountName} into ${vaultName}`,
  );
}

const revenueAccounts = readFileSync(
  new URL("../programs/nest-core/src/accounts/revenue.rs", import.meta.url),
  "utf8",
);
assert.match(
  revenueAccounts,
  /pub struct DepositProtocolRevenue[\s\S]*?seeds = \[b"protocol"\][\s\S]*?address = protocol\.nusd_mint[\s\S]*?token::authority = source[\s\S]*?address = protocol\.insurance_nusd_vault[\s\S]*?address = protocol\.staker_revenue_nusd_vault[\s\S]*?address = protocol\.protocol_revenue_nusd_vault[\s\S]*?pub source: Signer/,
  "external revenue must bind the canonical protocol, nUSD mint, source authority, and revenue vaults",
);

console.log("nest_core revenue surface checks passed");

function assertStakerDeltaGuard(path, amountName) {
  const source = sources.get(path);
  assert.ok(source, `missing source ${path}`);
  const guardedTransferPattern = new RegExp(
    [
      `if\\s+${escapeRegex(amountName)}\\s*>\\s*0\\s*\\{`,
      "let\\s+staker_revenue_before\\s*=\\s*ctx\\.accounts\\.staker_revenue_nusd_vault\\.amount;",
      "ctx\\.accounts\\.staker_revenue_nusd_vault\\.to_account_info\\(\\)",
      "ctx\\.accounts\\.staker_revenue_nusd_vault\\.reload\\(\\)\\?",
      "require_token_account_increase\\(",
      "staker_revenue_before,",
      "ctx\\.accounts\\.staker_revenue_nusd_vault\\.amount,",
      `${escapeRegex(amountName)},`,
    ].join("[\\s\\S]*?"),
  );
  assert.ok(
    guardedTransferPattern.test(source),
    `${path} must exact-check ${amountName} into staker_revenue_nusd_vault`,
  );
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
