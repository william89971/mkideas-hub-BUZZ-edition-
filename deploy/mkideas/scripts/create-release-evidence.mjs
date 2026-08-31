import { createHash, randomUUID } from "node:crypto";
import { readdir, readFile, stat, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const [artifactArg, outputArg] = process.argv.slice(2);
if (!artifactArg || !outputArg) {
  console.error("usage: node create-release-evidence.mjs <artifact-dir> <output-dir>");
  process.exit(64);
}

const root = process.cwd();
const artifactDir = path.resolve(artifactArg);
const outputDir = path.resolve(outputArg);

async function filesUnder(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const full = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...(await filesUnder(full)));
    if (entry.isFile()) result.push(full);
  }
  return result;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function packageJsonComponents() {
  const manifests = ["package.json", "desktop/package.json", "web/package.json", "admin-web/package.json"];
  const components = [];
  for (const manifest of manifests) {
    try {
      const json = JSON.parse(await readFile(path.join(root, manifest), "utf8"));
      for (const section of ["dependencies", "devDependencies", "optionalDependencies"]) {
        for (const [name, version] of Object.entries(json[section] ?? {})) {
          components.push({ type: "library", name, version: String(version), purl: `pkg:npm/${encodeURIComponent(name)}@${encodeURIComponent(String(version))}` });
        }
      }
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  return components;
}

async function cargoComponents() {
  const lock = await readFile(path.join(root, "Cargo.lock"), "utf8");
  return [...lock.matchAll(/\[\[package\]\]\s+name = "([^"]+)"\s+version = "([^"]+)"/g)].map((match) => ({
    type: "library",
    name: match[1],
    version: match[2],
    purl: `pkg:cargo/${encodeURIComponent(match[1])}@${encodeURIComponent(match[2])}`,
  }));
}

async function dartComponents() {
  const lock = await readFile(path.join(root, "mobile/pubspec.lock"), "utf8");
  const components = [];
  const lines = lock.split(/\r?\n/);
  let name = null;
  for (const line of lines) {
    const packageMatch = line.match(/^  ([a-zA-Z0-9_+-]+):\s*$/);
    if (packageMatch) name = packageMatch[1];
    const versionMatch = line.match(/^    version: "?([^"\s]+)"?\s*$/);
    if (name && versionMatch) {
      components.push({ type: "library", name, version: versionMatch[1], purl: `pkg:pub/${name}@${versionMatch[1]}` });
      name = null;
    }
  }
  return components;
}

const artifactFiles = (await filesUnder(artifactDir)).sort();
const artifacts = [];
for (const file of artifactFiles) {
  const bytes = await readFile(file);
  const details = await stat(file);
  artifacts.push({
    path: path.relative(artifactDir, file).replaceAll("\\", "/"),
    size: details.size,
    sha256: sha256(bytes),
  });
}

await mkdir(outputDir, { recursive: true });
await writeFile(
  path.join(outputDir, "SHA256SUMS"),
  `${artifacts.map((item) => `${item.sha256}  ${item.path}`).join("\n")}\n`,
);
await writeFile(
  path.join(outputDir, "release-manifest.json"),
  `${JSON.stringify({
    schemaVersion: 1,
    product: "MK Ideas Buzz",
    sourceRepository: process.env.GITHUB_REPOSITORY ?? "local",
    sourceRevision: process.env.GITHUB_SHA ?? "local",
    generatedAt: new Date().toISOString(),
    publishingAuthorized: false,
    artifacts,
  }, null, 2)}\n`,
);

const componentMap = new Map();
for (const component of [...(await cargoComponents()), ...(await packageJsonComponents()), ...(await dartComponents())]) {
  componentMap.set(component.purl, component);
}
const sbom = {
  bomFormat: "CycloneDX",
  specVersion: "1.6",
  serialNumber: `urn:uuid:${randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    component: { type: "application", name: "mkideas-buzz", version: process.env.GITHUB_SHA ?? "local" },
    properties: [
      { name: "mkideas:evidence-scope", value: "source dependency inventory; verify packaged binaries separately before release" },
    ],
  },
  components: [...componentMap.values()].sort((a, b) => a.purl.localeCompare(b.purl)),
};
await writeFile(path.join(outputDir, "source-sbom.cdx.json"), `${JSON.stringify(sbom, null, 2)}\n`);
console.log(`Wrote evidence for ${artifacts.length} artifacts and ${sbom.components.length} dependencies.`);

