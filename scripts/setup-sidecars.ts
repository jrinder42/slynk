import { existsSync, mkdirSync, chmodSync, unlinkSync, renameSync } from "fs";
import { join } from "path";
import { spawnSync } from "child_process";

const RCLONE_VERSION = "1.66.0";
const BINARIES_DIR = join(import.meta.dir, "..", "src-tauri", "binaries");

const platformMap = {
  darwin: "osx",
  linux: "linux",
  win32: "windows",
};

const archMap = {
  arm64: "arm64",
  x64: "amd64",
};

async function setupRclone() {
  const platform = process.platform;
  const arch = process.arch;

  const rclonePlatform = platformMap[platform];
  const rcloneArch = archMap[arch];

  if (!rclonePlatform || !rcloneArch) {
    console.error(`Unsupported platform or architecture: ${platform} ${arch}`);
    process.exit(1);
  }

  // Determine the Tauri target triple
  let triple = "";
  if (platform === "darwin") {
    triple = arch === "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin";
  } else if (platform === "win32") {
    triple = arch === "x64" ? "x86_64-pc-windows-msvc" : "aarch64-pc-windows-msvc";
  } else if (platform === "linux") {
    triple = arch === "arm64" ? "aarch64-unknown-linux-gnu" : "x86_64-unknown-linux-gnu";
  }

  const binaryName = `rclone-${triple}${platform === "win32" ? ".exe" : ""}`;
  const binaryPath = join(BINARIES_DIR, binaryName);

  if (existsSync(binaryPath)) {
    console.log(`✅ rclone sidecar already exists at ${binaryPath}`);
    return;
  }

  console.log(`🚀 Downloading rclone v${RCLONE_VERSION} for ${platform}-${arch}...`);

  if (!existsSync(BINARIES_DIR)) {
    mkdirSync(BINARIES_DIR, { recursive: true });
  }

  const url = `https://downloads.rclone.org/v${RCLONE_VERSION}/rclone-v${RCLONE_VERSION}-${rclonePlatform}-${rcloneArch}.zip`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to download rclone: ${response.statusText}`);
  }

  const zipPath = join(BINARIES_DIR, "rclone.zip");
  const arrayBuffer = await response.arrayBuffer();
  await Bun.write(zipPath, arrayBuffer);

  console.log(`📦 Extracting rclone...`);

  if (platform === "win32") {
    // Windows: Use PowerShell to extract
    spawnSync("powershell", [
      "-Command",
      `Expand-Archive -Path "${zipPath}" -DestinationPath "${BINARIES_DIR}" -Force`,
    ]);
    // Move the exe out of the nested folder if rclone zips it that way
    const nestedPath = join(BINARIES_DIR, `rclone-v${RCLONE_VERSION}-${rclonePlatform}-${rcloneArch}`, "rclone.exe");
    if (existsSync(nestedPath)) {
      renameSync(nestedPath, binaryPath);
    } else {
      // Sometimes rclone zips are flat or structured differently
      const flatPath = join(BINARIES_DIR, "rclone.exe");
      if (existsSync(flatPath)) renameSync(flatPath, binaryPath);
    }
  } else {
    // macOS/Linux: Use 'unzip'
    spawnSync("unzip", ["-j", zipPath, `**/rclone`, "-d", BINARIES_DIR]);
    const extractedPath = join(BINARIES_DIR, "rclone");
    if (existsSync(extractedPath)) {
      renameSync(extractedPath, binaryPath);
    }
  }

  // Cleanup
  if (existsSync(zipPath)) unlinkSync(zipPath);
  
  // Clean up any empty nested folders from extraction
  const nestedFolder = join(BINARIES_DIR, `rclone-v${RCLONE_VERSION}-${rclonePlatform}-${rcloneArch}`);
  if (existsSync(nestedFolder)) {
     spawnSync(platform === "win32" ? "rmdir" : "rm", ["-rf", nestedFolder]);
  }

  if (platform !== "win32") {
    chmodSync(binaryPath, 0o755);
  }

  console.log(`✨ rclone sidecar setup complete: ${binaryPath}`);
}

setupRclone().catch(console.error);
