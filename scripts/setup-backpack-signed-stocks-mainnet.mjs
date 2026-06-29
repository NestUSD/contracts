import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";

const contractsRoot = process.cwd();
const repoRoot = path.resolve(contractsRoot, "..");
const requireFromContracts = createRequire(path.join(contractsRoot, "package.json"));
const requireFromV2 = createRequire(path.join(repoRoot, "v2", "package.json"));

const anchor = requireFromContracts("@coral-xyz/anchor");
const { Connection, Keypair, PublicKey, SystemProgram } = requireFromContracts("@solana/web3.js");
const { createAccount, getMint, TOKEN_2022_PROGRAM_ID } = requireFromV2("@solana/spl-token");

const deploymentPath = path.join(repoRoot, "deployments", "mainnet-v1.json");
const idlPath = path.join(contractsRoot, "target", "idl", "nest_core.json");
const keypairPath = path.join(repoRoot, "mainnet-secrets", "nest-mainnet-deployer.json");
const artifactPath = path.join(repoRoot, "deployments", "mainnet-backpack-signed-stocks-2026-06-29.json");

const execute = process.argv.includes("--execute");
const skipSigner = process.argv.includes("--skip-set-signer");
const priceSigner = new PublicKey(
  process.env.NEST_PRICE_SIGNER_PUBLIC_KEY ?? "FuHvd8HNcunDdgxHseqXQK3ezkscyUrmakW5KYYfonnz",
);

const ZERO_FEED_ID = "0000000000000000000000000000000000000000000000000000000000000000";
const TOKEN_2022 = TOKEN_2022_PROGRAM_ID.toBase58();
const ASSETS = [
  {
    symbol: "MU",
    displaySymbol: "MU",
    issuer: "Backpack",
    assetFamily: "MU",
    mint: "MUxEsUKSMACyw5fZf68wxf5FLnZVhtU9CwH8uNNGay1",
    signedOracleFeedId: "136af1ad6026ced6f68a3a2f70b36d66eee910a6a61033226e8cb9db3e133a26",
    jupiterPriceId: "MU",
  },
  {
    symbol: "SNDK",
    displaySymbol: "SNDK",
    issuer: "Backpack",
    assetFamily: "SNDK",
    mint: "SNDKbwMUQvZhnLnxLduradgLHG5KrPuKwpnrkkGRhfH",
    signedOracleFeedId: "393e58c1b8491a8e63d662dc9d4b4c1bbef9a0f60ddd65631b8a8222e4038bbf",
    jupiterPriceId: "SNDK",
  },
  {
    symbol: "DRAM",
    displaySymbol: "DRAM",
    issuer: "Backpack",
    assetFamily: "DRAM",
    mint: "DRAMjSWR7HRfJKjRkvQWYL2bcaejaVhuxEcjf4pAY4Cw",
    signedOracleFeedId: "be9d1b1f73cb009ab1152b3445c8d8ee70496552add0482e99c11adcb1505e7f",
    jupiterPriceId: "DRAM",
  },
];

const POLICY = {
  borrowLtvBps: 4000,
  liquidationThresholdBps: 5000,
  liquidationPenaltyBps: 800,
  closeFactorBps: 5000,
  maxConfidenceBps: 200,
  maxStalenessSeconds: 120,
  closedMarketMaxStalenessSeconds: 86400,
  underlyingClosedMarketMaxStalenessSeconds: 432000,
  closedMarketHaircutBps: 9500,
  perVaultDebtCap: "1000000000000",
  protocolDebtCap: "1000000000000",
  depositCapRaw: "2500000000000",
};

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function keypairFromJson(filePath) {
  return Keypair.fromSecretKey(Uint8Array.from(readJson(filePath)));
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
    closedMarketMaxStalenessSeconds: params.closedMarketMaxStalenessSeconds.toString(),
    underlyingClosedMarketMaxStalenessSeconds: params.underlyingClosedMarketMaxStalenessSeconds.toString(),
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
    ...POLICY,
  };
}

const deployment = readJson(deploymentPath);
const idl = readJson(idlPath);
requireInstruction(idl, "set_nest_price_signer");
requireInstruction(idl, "add_collateral");

const payer = keypairFromJson(keypairPath);
const connection = new Connection(
  process.env.NEST_MAINNET_RPC
    ?? process.env.NEXT_PUBLIC_NEST_RPC_ENDPOINT
    ?? deployment.rpcEndpoint
    ?? "https://api.mainnet-beta.solana.com",
  "confirmed",
);
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), {
  commitment: "confirmed",
  preflightCommitment: "confirmed",
});
const program = new anchor.Program(idl, provider);
const protocol = new PublicKey(deployment.pdas.protocol);
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
      policy: POLICY,
      assets: {},
      signatures: {},
    };

console.log("rpc", connection.rpcEndpoint);
console.log("payer", payer.publicKey.toBase58());
console.log("program", program.programId.toBase58());
console.log("protocol", protocol.toBase58());
console.log("priceSigner", priceSigner.toBase58());
console.log("nestPriceSigner", nestPriceSigner.toBase58());
console.log(execute ? "mode execute" : "mode dry-run");

if (!skipSigner) {
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
    const info = await connection.getAccountInfo(new PublicKey(saved.address));
    if (info) return new PublicKey(saved.address);
  }

  if (!execute) {
    return null;
  }

  const vault = Keypair.generate();
  console.log(`creating ${assetRecord.symbol} ${label}`, vault.publicKey.toBase58());
  const signature = await createAccount(
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
    createSignature: signature,
  };
  writeJson(artifactPath, artifact);
  console.log(`${assetRecord.symbol} ${label} created`, signature);
  return vault.publicKey;
}

for (const asset of ASSETS) {
  const mint = new PublicKey(asset.mint);
  const [collateralConfig] = PublicKey.findProgramAddressSync(
    [Buffer.from("collateral"), mint.toBuffer()],
    program.programId,
  );
  const existingConfig = await connection.getAccountInfo(collateralConfig);
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
  artifact.assets[asset.symbol] = assetRecord;

  console.log(asset.symbol, "mint", mint.toBase58(), "decimals", mintInfo.decimals);
  console.log(asset.symbol, "collateralConfig", collateralConfig.toBase58());

  if (existingConfig) {
    console.log(asset.symbol, "collateral config already exists; skipping add_collateral");
    assetRecord.existsOnChain = true;
    writeJson(artifactPath, artifact);
    continue;
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
    underlyingUsdFeedId: feedIdBytes(ZERO_FEED_ID),
    redemptionRateFeedId: feedIdBytes(ZERO_FEED_ID),
    borrowLtvBps: POLICY.borrowLtvBps,
    liquidationThresholdBps: POLICY.liquidationThresholdBps,
    liquidationPenaltyBps: POLICY.liquidationPenaltyBps,
    closeFactorBps: POLICY.closeFactorBps,
    maxConfidenceBps: POLICY.maxConfidenceBps,
    maxStalenessSeconds: new anchor.BN(POLICY.maxStalenessSeconds),
    closedMarketMaxStalenessSeconds: new anchor.BN(POLICY.closedMarketMaxStalenessSeconds),
    underlyingClosedMarketMaxStalenessSeconds: new anchor.BN(POLICY.underlyingClosedMarketMaxStalenessSeconds),
    closedMarketHaircutBps: POLICY.closedMarketHaircutBps,
    perVaultDebtCap: new anchor.BN(POLICY.perVaultDebtCap),
    protocolDebtCap: new anchor.BN(POLICY.protocolDebtCap),
    depositCapRaw: new anchor.BN(POLICY.depositCapRaw),
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
    .signers([payer])
    .rpc();

  assetRecord.signatures.addCollateral = signature;
  writeJson(artifactPath, artifact);
  console.log(asset.symbol, "add_collateral confirmed", signature);
}

artifact.completedAt = execute ? new Date().toISOString() : undefined;
writeJson(artifactPath, artifact);
console.log("artifact", artifactPath);
