import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";

const contractsRoot = process.cwd();
const repoRoot = path.resolve(contractsRoot, "..");
const dependencyRoot = process.env.NEST_ADMIN_PACKAGE_ROOT
  ? path.resolve(process.env.NEST_ADMIN_PACKAGE_ROOT)
  : contractsRoot;
const requireFromContracts = createRequire(path.join(dependencyRoot, "package.json"));

const anchor = requireFromContracts("@coral-xyz/anchor");
const bs58Module = requireFromContracts("bs58");
const { Connection, Keypair, PublicKey, SystemProgram } = requireFromContracts("@solana/web3.js");
const { createAccount, getMint, TOKEN_2022_PROGRAM_ID } = requireFromContracts("@solana/spl-token");
const bs58 = bs58Module.default ?? bs58Module;

const deploymentPath = resolveRuntimePath(
  process.env.NEST_DEPLOYMENT_MANIFEST,
  path.join(repoRoot, "deployments", "mainnet-v1.json"),
);
const idlPath = resolveRuntimePath(
  process.env.NEST_CORE_IDL,
  path.join(contractsRoot, "target", "idl", "nest_core.json"),
);
const artifactPath = resolveRuntimePath(
  process.env.NEST_ONBOARDING_ARTIFACT,
  path.join(repoRoot, "deployments", "mainnet-backpack-signed-stocks-2026-06-29.json"),
);

const execute = process.argv.includes("--execute");
const setSigner = process.argv.includes("--set-price-signer");
const unpauseExisting = process.argv.includes("--unpause-existing");
const priceSigner = new PublicKey(
  process.env.NEST_PRICE_SIGNER_PUBLIC_KEY ?? "FuHvd8HNcunDdgxHseqXQK3ezkscyUrmakW5KYYfonnz",
);

const ZERO_FEED_ID = "0000000000000000000000000000000000000000000000000000000000000000";
const TOKEN_2022 = TOKEN_2022_PROGRAM_ID.toBase58();
const COMMON_POLICY = {
  borrowLtvBps: 4000,
  liquidationThresholdBps: 5000,
  liquidationPenaltyBps: 800,
  closeFactorBps: 5000,
  maxConfidenceBps: 200,
  maxStalenessSeconds: 120,
};
const LEGACY_POLICY = {
  ...COMMON_POLICY,
  perVaultDebtCap: "1000000000000",
  protocolDebtCap: "1000000000000",
  depositCapRaw: "2500000000000",
};
const ASSETS = [
  {
    symbol: "MU",
    displaySymbol: "MU",
    issuer: "Backpack",
    assetFamily: "MU",
    mint: "MUxEsUKSMACyw5fZf68wxf5FLnZVhtU9CwH8uNNGay1",
    signedOracleFeedId: "136af1ad6026ced6f68a3a2f70b36d66eee910a6a61033226e8cb9db3e133a26",
    jupiterPriceId: "MU",
    policy: LEGACY_POLICY,
  },
  {
    symbol: "SNDK",
    displaySymbol: "SNDK",
    issuer: "Backpack",
    assetFamily: "SNDK",
    mint: "SNDKbwMUQvZhnLnxLduradgLHG5KrPuKwpnrkkGRhfH",
    signedOracleFeedId: "393e58c1b8491a8e63d662dc9d4b4c1bbef9a0f60ddd65631b8a8222e4038bbf",
    jupiterPriceId: "SNDK",
    policy: LEGACY_POLICY,
  },
  {
    symbol: "DRAM",
    displaySymbol: "DRAM",
    issuer: "Backpack",
    assetFamily: "DRAM",
    mint: "DRAMjSWR7HRfJKjRkvQWYL2bcaejaVhuxEcjf4pAY4Cw",
    signedOracleFeedId: "be9d1b1f73cb009ab1152b3445c8d8ee70496552add0482e99c11adcb1505e7f",
    jupiterPriceId: "DRAM",
    policy: LEGACY_POLICY,
  },
  {
    symbol: "BOT",
    displaySymbol: "BOT",
    issuer: "Backpack",
    assetFamily: "BOT",
    mint: "BoTx8y9ynfdxf5ZjWtCoBVkff52qKA82ysaLU8ZM6d8T",
    signedOracleFeedId: "35694d41d084af891f5daa00fa6e240fa4d659bc110dbc6e53236ed5a55b930d",
    jupiterPriceId: "BoTx8y9ynfdxf5ZjWtCoBVkff52qKA82ysaLU8ZM6d8T",
    policy: {
      ...COMMON_POLICY,
      perVaultDebtCap: "10000000000",
      protocolDebtCap: "20000000000",
      depositCapRaw: "1800000000",
    },
  },
  {
    symbol: "SKHY",
    displaySymbol: "SKHY",
    issuer: "Backpack",
    assetFamily: "SKHY",
    mint: "SKHYhSjuRWHgikq8eRKbtBbpABgJSkd7ytQV14i9EQ3",
    signedOracleFeedId: "82090b7b19b86148b3718fe05c14a1a6ba7b0e0762ff396b349ebe3017745c60",
    jupiterPriceId: "SKHYhSjuRWHgikq8eRKbtBbpABgJSkd7ytQV14i9EQ3",
    policy: {
      ...COMMON_POLICY,
      perVaultDebtCap: "10000000000",
      protocolDebtCap: "20000000000",
      depositCapRaw: "300000000",
    },
  },
  {
    symbol: "HOOD",
    displaySymbol: "HOOD",
    issuer: "Backpack",
    assetFamily: "HOOD",
    mint: "HooDYv5RewLRiMLnEVq3VJqdqxhuE6c5eYvqejMC3e9A",
    signedOracleFeedId: "33c2697b3d4cd799c88bbe4fc181ee391c22c38d424f429374e3f9cdb452af98",
    jupiterPriceId: "HooDYv5RewLRiMLnEVq3VJqdqxhuE6c5eYvqejMC3e9A",
    policy: {
      ...COMMON_POLICY,
      perVaultDebtCap: "10000000000",
      protocolDebtCap: "20000000000",
      depositCapRaw: "475000000",
    },
  },
  {
    symbol: "INTC",
    displaySymbol: "INTC",
    issuer: "Backpack",
    assetFamily: "INTC",
    mint: "iNTCy1qTsUEZQe3DSocLz1ZXXai34Gdw8THQh5rxFaF",
    signedOracleFeedId: "6e81bd30b345a3e54de4fc556facbdbc16f3d659a4851139f32e6df3eaca5fd6",
    jupiterPriceId: "iNTCy1qTsUEZQe3DSocLz1ZXXai34Gdw8THQh5rxFaF",
    policy: {
      ...COMMON_POLICY,
      perVaultDebtCap: "10000000000",
      protocolDebtCap: "20000000000",
      depositCapRaw: "475000000",
    },
  },
];
const requestedSymbols = new Set(
  (optionValue("--assets") ?? ASSETS.map((asset) => asset.symbol).join(","))
    .split(",")
    .map((symbol) => symbol.trim().toUpperCase())
    .filter(Boolean),
);
const knownSymbols = new Set(ASSETS.map((asset) => asset.symbol));
for (const symbol of requestedSymbols) {
  if (!knownSymbols.has(symbol)) throw new Error(`unknown asset requested: ${symbol}`);
}
const selectedAssets = ASSETS.filter((asset) => requestedSymbols.has(asset.symbol));
if (selectedAssets.length === 0) throw new Error("at least one asset must be selected");
if (unpauseExisting && !execute) throw new Error("--unpause-existing requires --execute");

function optionValue(name) {
  const prefix = `${name}=`;
  const argument = process.argv.find((value) => value.startsWith(prefix));
  return argument?.slice(prefix.length);
}

function resolveRuntimePath(value, fallback) {
  if (!value) return fallback;
  return path.isAbsolute(value) ? value : path.resolve(dependencyRoot, value);
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function keypairFromEnvironment() {
  const encoded = process.env.NEST_ADMIN_PRIVATE_KEY_B58
    ?? process.env.NEST_CORE_AUTHORITY_PRIVATE_KEY_B58;
  if (!encoded) {
    throw new Error("NEST_ADMIN_PRIVATE_KEY_B58 or NEST_CORE_AUTHORITY_PRIVATE_KEY_B58 is required");
  }
  const decoded = bs58.decode(encoded.trim());
  if (decoded.length !== 64) throw new Error("Core authority private key must decode to 64 bytes");
  return Keypair.fromSecretKey(decoded);
}

function symbolBytes(value) {
  const bytes = Buffer.alloc(16);
  const source = Buffer.from(value, "utf8");
  if (source.length > bytes.length) throw new Error(`symbol too long: ${value}`);
  source.copy(bytes);
  return [...bytes];
}

function feedIdBytes(hex) {
  const bytes = Buffer.from(hex, "hex");
  if (bytes.length !== 32) throw new Error(`invalid feed id: ${hex}`);
  return [...bytes];
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function getAccountInfoWithRetry(address, attempts = 4) {
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const info = await connection.getAccountInfo(address, "confirmed");
    if (info) return info;
    if (attempt < attempts) await delay(750);
  }
  return null;
}

async function fetchCollateralConfigWithRetry(address, attempts = 16) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await program.account.collateralConfig.fetch(address);
    } catch (error) {
      lastError = error;
      if (attempt < attempts) await delay(750);
    }
  }
  throw lastError;
}

function requireInstruction(idl, name) {
  if (!idl.instructions.some((item) => item.name === name)) {
    throw new Error(`local IDL is missing ${name}; run npm run build:anchor first`);
  }
}

function serializeParams(params) {
  return {
    ...params,
    collateralMint: params.collateralMint.toBase58(),
    tokenProgram: params.tokenProgram.toBase58(),
    maxStalenessSeconds: params.maxStalenessSeconds.toString(),
    perVaultDebtCap: params.perVaultDebtCap.toString(),
    protocolDebtCap: params.protocolDebtCap.toString(),
    depositCapRaw: params.depositCapRaw.toString(),
  };
}

function manifestEntry(asset, addresses, decimals) {
  return {
    symbol: asset.symbol,
    displaySymbol: asset.displaySymbol,
    issuer: asset.issuer,
    assetFamily: asset.assetFamily,
    mint: asset.mint,
    collateralConfig: addresses.collateralConfig,
    collateralVault: addresses.collateralVault,
    insuranceCollateralVault: addresses.insuranceCollateralVault,
    tokenProgram: TOKEN_2022,
    collateralDecimals: decimals,
    xstockUsdFeedId: asset.signedOracleFeedId,
    signedOracleFeedId: asset.signedOracleFeedId,
    jupiterPriceId: asset.jupiterPriceId,
    oracleProvider: "jupiter-signed",
    underlyingUsdFeedId: ZERO_FEED_ID,
    redemptionRateFeedId: ZERO_FEED_ID,
    ...asset.policy,
  };
}

function assertConfigMatches(asset, config, mint, decimals) {
  const checks = [
    [config.collateralMint.equals(mint), "collateral mint"],
    [config.tokenProgram.equals(TOKEN_2022_PROGRAM_ID), "token program"],
    [config.collateralDecimals === decimals, "collateral decimals"],
    [Buffer.from(config.xstockUsdFeedId).toString("hex") === asset.signedOracleFeedId, "oracle feed id"],
    [config.borrowLtvBps === asset.policy.borrowLtvBps, "borrow LTV"],
    [config.liquidationThresholdBps === asset.policy.liquidationThresholdBps, "liquidation threshold"],
    [config.liquidationPenaltyBps === asset.policy.liquidationPenaltyBps, "liquidation penalty"],
    [config.closeFactorBps === asset.policy.closeFactorBps, "close factor"],
    [config.maxConfidenceBps === asset.policy.maxConfidenceBps, "maximum confidence"],
    [config.maxStalenessSeconds.toString() === String(asset.policy.maxStalenessSeconds), "maximum staleness"],
    [config.perVaultDebtCap.toString() === asset.policy.perVaultDebtCap, "per-vault debt cap"],
    [config.protocolDebtCap.toString() === asset.policy.protocolDebtCap, "market debt cap"],
    [config.depositCapRaw.toString() === asset.policy.depositCapRaw, "deposit cap"],
  ];
  const mismatch = checks.find(([matches]) => !matches);
  if (mismatch) throw new Error(`${asset.symbol}: on-chain ${mismatch[1]} does not match the reviewed policy`);
}

const deployment = readJson(deploymentPath);
const idl = readJson(idlPath);
requireInstruction(idl, "set_nest_price_signer");
requireInstruction(idl, "add_collateral");
requireInstruction(idl, "set_collateral_paused");

const rpcEndpoint = (process.env.NEST_MAINNET_RPC ?? process.env.NEST_RPC_ENDPOINT)?.trim();
if (!rpcEndpoint) {
  throw new Error("NEST_MAINNET_RPC or NEST_RPC_ENDPOINT is required; public RPC fallbacks are disabled");
}
const payer = keypairFromEnvironment();
const connection = new Connection(rpcEndpoint, "confirmed");
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const program = new anchor.Program(idl, provider);
const protocol = new PublicKey(deployment.pdas.protocol);
const expectedProgram = new PublicKey(deployment.programs.nestCore);
if (!program.programId.equals(expectedProgram)) {
  throw new Error(`IDL program ${program.programId.toBase58()} does not match manifest ${expectedProgram.toBase58()}`);
}
const [nestPriceSigner] = PublicKey.findProgramAddressSync(
  [Buffer.from("nest_price_signer"), protocol.toBuffer()],
  program.programId,
);

const artifact = fs.existsSync(artifactPath)
  ? readJson(artifactPath)
  : {
      cluster: "mainnet-beta",
      createdAt: new Date().toISOString(),
      program: program.programId.toBase58(),
      protocol: protocol.toBase58(),
      payer: payer.publicKey.toBase58(),
      priceSigner: priceSigner.toBase58(),
      nestPriceSigner: nestPriceSigner.toBase58(),
      policies: Object.fromEntries(selectedAssets.map((asset) => [asset.symbol, asset.policy])),
      assets: {},
      signatures: {},
    };
artifact.policies = {
  ...artifact.policies,
  ...Object.fromEntries(selectedAssets.map((asset) => [asset.symbol, asset.policy])),
};

const genesisHash = await connection.getGenesisHash();
if (genesisHash !== "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d") {
  throw new Error(`configured RPC is not Solana mainnet-beta: ${genesisHash}`);
}
const protocolAccount = await program.account.protocol.fetch(protocol);
if (!protocolAccount.authority.equals(payer.publicKey)) {
  throw new Error("configured private key is not the on-chain Core authority");
}
const payerBalanceLamports = await connection.getBalance(payer.publicKey, "confirmed");
if (payerBalanceLamports < 100_000_000) {
  throw new Error("Core authority needs at least 0.1 SOL for collateral-account rent and transaction fees");
}

console.log("rpc configured via NEST_MAINNET_RPC");
console.log("payer", payer.publicKey.toBase58());
console.log("payerBalanceSol", (payerBalanceLamports / 1_000_000_000).toFixed(4));
console.log("program", program.programId.toBase58());
console.log("protocol", protocol.toBase58());
console.log("priceSigner", priceSigner.toBase58());
console.log("nestPriceSigner", nestPriceSigner.toBase58());
console.log("assets", selectedAssets.map((asset) => asset.symbol).join(","));
console.log(execute ? "mode execute" : "mode dry-run");

if (setSigner) {
  artifact.priceSigner = priceSigner.toBase58();
  artifact.nestPriceSigner = nestPriceSigner.toBase58();
  if (execute) {
    console.log("setting on-chain price signer");
    const signature = await program.methods
      .setNestPriceSigner(priceSigner)
      .accounts({
        protocol,
        nestPriceSigner,
        authority: payer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([payer])
      .rpc();
    artifact.signatures.setNestPriceSigner = signature;
    writeJson(artifactPath, artifact);
    console.log("set_nest_price_signer confirmed", signature);
  }
}

async function ensureVault(assetRecord, label, mint) {
  const saved = assetRecord.vaults?.[label];
  if (saved?.address) {
    const info = await getAccountInfoWithRetry(new PublicKey(saved.address));
    if (info) return new PublicKey(saved.address);
  }

  if (!execute) {
    return null;
  }

  const vault = Keypair.generate();
  console.log(`creating ${assetRecord.symbol} ${label}`, vault.publicKey.toBase58());
  const createdAddress = await createAccount(
    connection,
    payer,
    mint,
    protocol,
    vault,
    { commitment: "confirmed" },
    TOKEN_2022_PROGRAM_ID,
  );
  assetRecord.vaults ??= {};
  assetRecord.vaults[label] = {
    address: vault.publicKey.toBase58(),
  };
  writeJson(artifactPath, artifact);
  console.log(`${assetRecord.symbol} ${label} created`, createdAddress.toBase58());
  return createdAddress;
}

for (const asset of selectedAssets) {
  const mint = new PublicKey(asset.mint);
  const [collateralConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("collateral"), mint.toBuffer()],
    program.programId,
  );
  const existingConfig = await getAccountInfoWithRetry(collateralConfig);
  const mintInfo = await getMint(connection, mint, "confirmed", TOKEN_2022_PROGRAM_ID);

  const assetRecord = artifact.assets[asset.symbol] ?? {
    ...asset,
    mint: mint.toBase58(),
    collateralConfig: collateralConfig.toBase58(),
    tokenProgram: TOKEN_2022,
    collateralDecimals: mintInfo.decimals,
    vaults: {},
    signatures: {},
  };
  assetRecord.symbol = asset.symbol;
  assetRecord.collateralConfig = collateralConfig.toBase58();
  assetRecord.collateralDecimals = mintInfo.decimals;
  assetRecord.policy = asset.policy;
  artifact.assets[asset.symbol] = assetRecord;

  console.log(asset.symbol, "mint", mint.toBase58(), "decimals", mintInfo.decimals);
  console.log(asset.symbol, "collateralConfig", collateralConfig.toBase58());

  if (existingConfig) {
    const config = await fetchCollateralConfigWithRetry(collateralConfig);
    assertConfigMatches(asset, config, mint, mintInfo.decimals);
    assetRecord.existsOnChain = true;
    assetRecord.vaults = {
      collateralVault: { address: config.collateralVault.toBase58() },
      insuranceCollateralVault: { address: config.insuranceCollateralVault.toBase58() },
    };
    assetRecord.manifestEntry = manifestEntry(asset, {
      collateralConfig: collateralConfig.toBase58(),
      collateralVault: config.collateralVault.toBase58(),
      insuranceCollateralVault: config.insuranceCollateralVault.toBase58(),
    }, mintInfo.decimals);
    assetRecord.initiallyPaused = config.depositsPaused && config.borrowsPaused && config.withdrawsPaused;
    if (!assetRecord.signatures.addCollateral) {
      const [creation] = await connection.getSignaturesForAddress(collateralConfig, { limit: 1 }, "confirmed");
      if (creation && !creation.err) assetRecord.signatures.addCollateral = creation.signature;
    }
    if (unpauseExisting) {
      console.log(asset.symbol, "unpausing reviewed collateral config");
      const signature = await program.methods
        .setCollateralPaused(false, false, false)
        .accounts({ protocol, collateralConfig, authority: payer.publicKey })
        .signers([payer])
        .rpc();
      const activeConfig = await fetchCollateralConfigWithRetry(collateralConfig);
      assertConfigMatches(asset, activeConfig, mint, mintInfo.decimals);
      if (activeConfig.depositsPaused || activeConfig.borrowsPaused || activeConfig.withdrawsPaused) {
        throw new Error(`${asset.symbol}: collateral remained paused after activation`);
      }
      assetRecord.signatures.unpause = signature;
      assetRecord.initiallyPaused = false;
      console.log(asset.symbol, "unpause confirmed", signature);
    } else {
      console.log(asset.symbol, "collateral config already exists and matches policy");
    }
    writeJson(artifactPath, artifact);
    continue;
  }

  if (unpauseExisting) {
    throw new Error(`${asset.symbol}: cannot unpause a collateral config that does not exist`);
  }

  const collateralVault = await ensureVault(assetRecord, "collateralVault", mint);
  const insuranceCollateralVault = await ensureVault(assetRecord, "insuranceCollateralVault", mint);
  if (collateralVault && insuranceCollateralVault && collateralVault.equals(insuranceCollateralVault)) {
    throw new Error(`${asset.symbol}: collateral and insurance vaults must be distinct`);
  }

  const params = {
    collateralMint: mint,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
    symbol: symbolBytes(asset.symbol),
    collateralDecimals: mintInfo.decimals,
    xstockUsdFeedId: feedIdBytes(asset.signedOracleFeedId),
    borrowLtvBps: asset.policy.borrowLtvBps,
    liquidationThresholdBps: asset.policy.liquidationThresholdBps,
    liquidationPenaltyBps: asset.policy.liquidationPenaltyBps,
    closeFactorBps: asset.policy.closeFactorBps,
    maxConfidenceBps: asset.policy.maxConfidenceBps,
    maxStalenessSeconds: new anchor.BN(asset.policy.maxStalenessSeconds),
    perVaultDebtCap: new anchor.BN(asset.policy.perVaultDebtCap),
    protocolDebtCap: new anchor.BN(asset.policy.protocolDebtCap),
    depositCapRaw: new anchor.BN(asset.policy.depositCapRaw),
  };

  assetRecord.params = serializeParams(params);
  if (collateralVault && insuranceCollateralVault) {
    assetRecord.manifestEntry = manifestEntry(asset, {
      collateralConfig: collateralConfig.toBase58(),
      collateralVault: collateralVault.toBase58(),
      insuranceCollateralVault: insuranceCollateralVault.toBase58(),
    }, mintInfo.decimals);
  }
  writeJson(artifactPath, artifact);

  if (!execute) {
    console.log(asset.symbol, "dry run only; rerun with --execute after funding and upgrading");
    continue;
  }

  console.log(asset.symbol, "sending add_collateral");
  const pauseInstruction = await program.methods
    .setCollateralPaused(true, true, true)
    .accounts({
      protocol,
      collateralConfig,
      authority: payer.publicKey,
    })
    .instruction();
  const signature = await program.methods
    .addCollateral(params)
    .accounts({
      protocol,
      collateralConfig,
      collateralMint: mint,
      collateralVault,
      insuranceCollateralVault,
      authority: payer.publicKey,
      collateralTokenProgram: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .postInstructions([pauseInstruction])
    .signers([payer])
    .rpc();

  assetRecord.signatures.addCollateral = signature;
  writeJson(artifactPath, artifact);
  const createdConfig = await fetchCollateralConfigWithRetry(collateralConfig);
  if (!createdConfig.depositsPaused || !createdConfig.borrowsPaused || !createdConfig.withdrawsPaused) {
    throw new Error(`${asset.symbol}: collateral was created without all pause flags enabled`);
  }
  assetRecord.initiallyPaused = true;
  writeJson(artifactPath, artifact);
  console.log(asset.symbol, "add_collateral confirmed and paused", signature);
}

artifact.completedAt = execute ? new Date().toISOString() : undefined;
writeJson(artifactPath, artifact);
console.log("artifact", artifactPath);
