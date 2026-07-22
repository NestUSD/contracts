import assert from "node:assert/strict";
import fs from "node:fs";

const packageJson = JSON.parse(fs.readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const packageLock = JSON.parse(fs.readFileSync(new URL("../package-lock.json", import.meta.url), "utf8"));
const setupScript = fs.readFileSync(
  new URL("../scripts/setup-backpack-signed-stocks-mainnet.mjs", import.meta.url),
  "utf8",
);
const applyArtifactScript = fs.readFileSync(
  new URL("../scripts/apply-backpack-collateral-artifact.mjs", import.meta.url),
  "utf8",
);

const splTokenVersion = packageJson.devDependencies?.["@solana/spl-token"];
assert.match(splTokenVersion, /^\d+\.\d+\.\d+$/, "@solana/spl-token must use an exact version");
assert.equal(
  packageLock.packages?.[""]?.devDependencies?.["@solana/spl-token"],
  splTokenVersion,
  "package-lock must pin the same @solana/spl-token version",
);
assert.doesNotMatch(setupScript, /requireFromV2|createRequire\([^)]*v2/);
assert.match(
  setupScript,
  /requireFromContracts\("@solana\/spl-token"\)/,
  "the privileged setup script must resolve SPL Token from the audited contracts project",
);
assert.equal(packageJson.devDependencies?.bs58, "6.0.0", "bs58 must use an exact version");
assert.equal(
  packageLock.packages?.[""]?.devDependencies?.bs58,
  packageJson.devDependencies.bs58,
  "package-lock must pin the same bs58 version",
);
assert.match(
  setupScript,
  /NEST_ADMIN_PRIVATE_KEY_B58[\s\S]*?NEST_CORE_AUTHORITY_PRIVATE_KEY_B58/,
  "the setup script must load the authority from a base58 environment variable",
);
assert.doesNotMatch(setupScript, /keypairPath|mainnet-secrets|fromSecretKey\(Uint8Array\.from/);
assert.doesNotMatch(
  setupScript,
  /closedMarketMaxStalenessSeconds|underlyingClosedMarketMaxStalenessSeconds|closedMarketHaircutBps/,
  "the setup script must use the current add_collateral parameter shape",
);
assert.match(
  setupScript,
  /\.setCollateralPaused\(true, true, true\)[\s\S]*?\.postInstructions\(\[pauseInstruction\]\)/,
  "new collateral must be created paused in the same transaction",
);
assert.match(
  setupScript,
  /waitForCollateralConfig[\s\S]*?collateral config is already active/,
  "activation must tolerate RPC propagation lag and avoid resending an already-active market",
);

for (const [symbol, mint] of [
  ["BOT", "BoTx8y9ynfdxf5ZjWtCoBVkff52qKA82ysaLU8ZM6d8T"],
  ["SKHY", "SKHYhSjuRWHgikq8eRKbtBbpABgJSkd7ytQV14i9EQ3"],
  ["HOOD", "HooDYv5RewLRiMLnEVq3VJqdqxhuE6c5eYvqejMC3e9A"],
  ["INTC", "iNTCy1qTsUEZQe3DSocLz1ZXXai34Gdw8THQh5rxFaF"],
]) {
  assert.match(setupScript, new RegExp(`symbol: "${symbol}"[\\s\\S]*?mint: "${mint}"`));
}
assert.equal(
  setupScript.match(/perVaultDebtCap: "10000000000"/g)?.length,
  4,
  "each Sunrise market must cap individual debt at 10,000 nUSD",
);
assert.equal(
  setupScript.match(/protocolDebtCap: "20000000000"/g)?.length,
  4,
  "each Sunrise market must cap aggregate debt at 20,000 nUSD",
);
assert.match(applyArtifactScript, /already exists with different configuration/);
assert.match(applyArtifactScript, /manifest\.oracleAssets = manifest\.oracleAssets\.filter/);
