#!/usr/bin/env node
// Tarball proof for the @3leaps/ipcprims dual CJS/ESM package surface.
//
// Packs the root package and asserts the ESM facade plus all three declaration
// variants ship, no native-addon/platform-selection internals leak into the
// root tarball, and that Node ESM + CJS resolution against the packed surface
// succeeds (with the locally built addon staged beside it, since the real
// binary is supplied by the platform optional packages).
import { execFileSync, spawnSync } from "node:child_process";
import {
	copyFileSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	renameSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { fileURLToPath } from "node:url";
import os from "node:os";
import path from "node:path";

const dir = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(dir, "..");

const required = [
	"index.js",
	"index.mjs",
	"index.d.ts",
	"index.d.cts",
	"index.d.mts",
	"package.json",
	"README.md",
	"LICENSE-MIT",
	"LICENSE-APACHE",
];

function fail(msg) {
	console.error(`::error::${msg}`);
	process.exit(1);
}

const tmp = mkdtempSync(path.join(os.tmpdir(), "ipcprims-pack-"));
try {
	const packJson = execFileSync(
		"npm",
		["pack", "--json", "--ignore-scripts", "--pack-destination", tmp],
		{ cwd: root, encoding: "utf8" },
	);
	const pack = JSON.parse(packJson)[0];
	const tgz = path.join(tmp, pack.filename);
	const packedFiles = new Set(pack.files.map((f) => f.path));

	for (const f of required) {
		if (!packedFiles.has(f)) fail(`tarball missing ${f}`);
	}
	if (![...packedFiles].some((f) => f.startsWith("npm/"))) {
		fail("tarball missing npm/ platform manifests");
	}
	for (const f of packedFiles) {
		if (
			f.endsWith(".node") ||
			f.startsWith("src/") ||
			f.startsWith("scripts/")
		) {
			fail(`tarball unexpectedly ships ${f}`);
		}
	}

	const extractDir = path.join(tmp, "extract");
	mkdirSync(extractDir, { recursive: true });
	const tar = spawnSync("tar", ["-xzf", tgz, "-C", extractDir], {
		stdio: "inherit",
	});
	if (tar.status !== 0) fail("tar extraction failed");

	// Install the packed package under an external consumer's node_modules so
	// the smoke exercises the `exports` map and its import/require conditions
	// via real package specifiers, not relative paths that bypass it.
	const consumerDir = path.join(tmp, "consumer");
	const pkgDest = path.join(consumerDir, "node_modules", "@3leaps", "ipcprims");
	mkdirSync(path.dirname(pkgDest), { recursive: true });
	renameSync(path.join(extractDir, "package"), pkgDest);

	const localNode = readdirSync(root).find(
		(n) => n.startsWith("ipcprims.") && n.endsWith(".node"),
	);
	if (!localNode) fail("no locally built .node to stage for the tarball smoke");
	copyFileSync(path.join(root, localNode), path.join(pkgDest, localNode));

	const smoke = `
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const names = ["AsyncChannelReceiver","AsyncListener","AsyncPeer","Listener","Peer","SchemaRegistry","control","command","data","telemetry","error","CONTROL","COMMAND","DATA","TELEMETRY","ERROR"];
const expected = names.slice().sort().join(",");
const classes = ["AsyncChannelReceiver","AsyncListener","AsyncPeer","Listener","Peer","SchemaRegistry"];
const pairs = [["control","CONTROL"],["command","COMMAND"],["data","DATA"],["telemetry","TELEMETRY"],["error","ERROR"]];
const esmRoot = await import("@3leaps/ipcprims");
const esmIndex = await import("@3leaps/ipcprims/index");
const esmJs = await import("@3leaps/ipcprims/index.js");
const cjsRoot = require("@3leaps/ipcprims");
const cjsIndex = require("@3leaps/ipcprims/index");
const cjsJs = require("@3leaps/ipcprims/index.js");
const namespaces = { "esm-root": esmRoot, "esm-index": esmIndex, "esm-js": esmJs, "cjs-root": cjsRoot, "cjs-index": cjsIndex, "cjs-js": cjsJs };
for (const [label, ns] of Object.entries(namespaces)) {
  if (Object.keys(ns).sort().join(",") !== expected || "default" in ns) {
    console.error(label + " export mismatch: " + Object.keys(ns).sort().join(","));
    process.exit(1);
  }
  for (const c of classes) if (ns[c] !== esmRoot[c]) { console.error(label + " class identity mismatch: " + c); process.exit(1); }
  for (const [fn, cn] of pairs) if (ns[fn]() !== ns[cn]) { console.error(label + " constant equivalence mismatch: " + fn); process.exit(1); }
}
console.log("TARBALL-SMOKE-OK: 6 specifier/alias namespaces expose 16 names; 6 class identities + 5 constant equivalences hold across all");
`;
	writeFileSync(path.join(consumerDir, "smoke.mjs"), smoke);
	const run = spawnSync("node", [path.join(consumerDir, "smoke.mjs")], {
		encoding: "utf8",
	});
	if (run.status !== 0)
		fail(`tarball smoke failed: ${run.stderr || run.stdout}`);
	process.stdout.write(run.stdout);

	console.log(
		"OK: tarball ships the ESM facade + three declaration variants and no native internals; packed ESM/CJS resolution verified.",
	);
} finally {
	rmSync(tmp, { recursive: true, force: true });
}
