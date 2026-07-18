import assert from "node:assert/strict";
import fs from "node:fs";

const packageJson = JSON.parse(fs.readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const packageLock = JSON.parse(fs.readFileSync(new URL("../package-lock.json", import.meta.url), "utf8"));
const setupScript = fs.readFileSync(
  new URL("../scripts/setup-backpack-signed-stocks-mainnet.mjs", import.meta.url),
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
