import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const DEFAULT_SYMBOLS = ["BOT", "SKHY", "HOOD", "INTC"];
const artifactPath = requiredOption("--artifact");
const configPaths = optionValues("--config");
const symbols = (optionValue("--assets") ?? DEFAULT_SYMBOLS.join(","))
  .split(",")
  .map((value) => value.trim().toUpperCase())
  .filter(Boolean);

if (configPaths.length === 0) throw new Error("at least one --config path is required");
const artifact = readJson(artifactPath);
const entries = symbols.map((symbol) => validatedEntry(symbol, artifact.assets?.[symbol]?.manifestEntry));

for (const configPath of configPaths) {
  const manifest = readJson(configPath);
  if (!Array.isArray(manifest.collateral)) throw new Error(`${configPath}: collateral array is required`);
  const existing = new Map(manifest.collateral.map((entry) => [entry.symbol, entry]));
  for (const entry of entries) {
    const previous = existing.get(entry.symbol);
    if (previous && JSON.stringify(previous) !== JSON.stringify(entry)) {
      throw new Error(`${configPath}: ${entry.symbol} already exists with different configuration`);
    }
    if (!previous) manifest.collateral.push(entry);
  }
  if (Array.isArray(manifest.oracleAssets)) {
    manifest.oracleAssets = manifest.oracleAssets.filter((entry) => !symbols.includes(entry?.symbol));
  }
  fs.writeFileSync(configPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log("updated", path.resolve(configPath));
}

function validatedEntry(symbol, entry) {
  if (!entry || entry.symbol !== symbol) throw new Error(`${symbol}: artifact manifest entry is missing`);
  for (const field of [
    "mint",
    "collateralConfig",
    "collateralVault",
    "insuranceCollateralVault",
    "tokenProgram",
    "signedOracleFeedId",
    "jupiterPriceId",
  ]) {
    if (typeof entry[field] !== "string" || entry[field].length === 0) {
      throw new Error(`${symbol}: ${field} is required`);
    }
  }
  if (entry.oracleProvider !== "jupiter-signed") throw new Error(`${symbol}: invalid oracle provider`);
  if (entry.collateralDecimals !== 6) throw new Error(`${symbol}: collateral decimals must be 6`);
  if (entry.borrowLtvBps !== 4000 || entry.liquidationThresholdBps !== 5000) {
    throw new Error(`${symbol}: risk policy does not match the reviewed launch policy`);
  }
  if (entry.perVaultDebtCap !== "10000000000" || entry.protocolDebtCap !== "20000000000") {
    throw new Error(`${symbol}: debt caps do not match the reviewed launch policy`);
  }
  return entry;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function optionValues(name) {
  const prefix = `${name}=`;
  return process.argv.filter((value) => value.startsWith(prefix)).map((value) => value.slice(prefix.length));
}

function optionValue(name) {
  return optionValues(name)[0];
}

function requiredOption(name) {
  const value = optionValue(name);
  if (!value) throw new Error(`${name}=... is required`);
  return value;
}
