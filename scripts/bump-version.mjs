import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");

const packageJsonPath = path.join(rootDir, "package.json");
const tauriConfPath = path.join(rootDir, "src-tauri", "tauri.conf.json");
const cargoTomlPath = path.join(rootDir, "src-tauri", "Cargo.toml");

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf-8"));
}

function writeJson(filePath, data) {
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n", "utf-8");
}

const pkg = readJson(packageJsonPath);
const currentVersion = pkg.version || "0.1.0";

const arg = process.argv[2] || "patch";

function bumpVersion(version, type) {
  const parts = version.split(".").map(Number);
  if (parts.length !== 3 || parts.some(isNaN)) {
    throw new Error(`Invalid semantic version: ${version}`);
  }

  let [major, minor, patch] = parts;

  if (type === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (type === "minor") {
    minor += 1;
    patch = 0;
  } else if (type === "patch") {
    patch += 1;
  } else if (/^\d+\.\d+\.\d+$/.test(type)) {
    return type;
  } else {
    throw new Error(`Unknown bump type: ${type}. Use 'patch', 'minor', 'major', or a specific version like '0.1.2'.`);
  }

  return `${major}.${minor}.${patch}`;
}

const newVersion = bumpVersion(currentVersion, arg);
console.log(`Bumping version: ${currentVersion} -> ${newVersion}`);

// 1. Update package.json
pkg.version = newVersion;
writeJson(packageJsonPath, pkg);
console.log(`✓ Updated package.json`);

// 2. Update src-tauri/tauri.conf.json
if (fs.existsSync(tauriConfPath)) {
  const tauriConf = readJson(tauriConfPath);
  tauriConf.version = newVersion;
  writeJson(tauriConfPath, tauriConf);
  console.log(`✓ Updated src-tauri/tauri.conf.json`);
}

// 3. Update src-tauri/Cargo.toml (only the [package] version)
if (fs.existsSync(cargoTomlPath)) {
  let cargoToml = fs.readFileSync(cargoTomlPath, "utf-8");
  // Replace version under [package]
  cargoToml = cargoToml.replace(
    /(\[package\][\s\S]*?\nversion\s*=\s*)"[^"]+"/,
    `$1"${newVersion}"`
  );
  fs.writeFileSync(cargoTomlPath, cargoToml, "utf-8");
  console.log(`✓ Updated src-tauri/Cargo.toml`);
}

console.log(`\nSuccess! Version is now ${newVersion} across all project files.`);
