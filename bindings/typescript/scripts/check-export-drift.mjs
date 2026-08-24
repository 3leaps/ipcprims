#!/usr/bin/env node
// Export-drift guard for the @3leaps/ipcprims dual CJS/ESM package surface.
//
// Asserts, in one pass, that the declared value space matches the runtime and
// that the CJS and ESM entry points expose the same named exports with shared
// class/prototype identity and a single native binding instance. Kept
// value-space only: type-only declarations (interfaces) must compile but are
// not runtime-parity candidates.
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const require = createRequire(import.meta.url);
const ts = require("typescript");

const dir = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(dir, "..");

const values = [
	"AsyncChannelReceiver",
	"AsyncListener",
	"AsyncPeer",
	"Listener",
	"Peer",
	"SchemaRegistry",
	"control",
	"command",
	"data",
	"telemetry",
	"error",
	"CONTROL",
	"COMMAND",
	"DATA",
	"TELEMETRY",
	"ERROR",
].sort();
const types = [
	"AuthTokenResult",
	"JsFrame",
	"ListenerOptions",
	"RecvAsyncOptions",
].sort();
const classNames = [
	"AsyncChannelReceiver",
	"AsyncListener",
	"AsyncPeer",
	"Listener",
	"Peer",
	"SchemaRegistry",
];
const functions = ["control", "command", "data", "telemetry", "error"];
const constants = ["CONTROL", "COMMAND", "DATA", "TELEMETRY", "ERROR"];

function fail(msg) {
	console.error(`::error::${msg}`);
	process.exit(1);
}

function parseDeclaration(file) {
	const text = readFileSync(file, "utf8");
	const source = ts.createSourceFile(
		file,
		text,
		ts.ScriptTarget.Latest,
		true,
		ts.ScriptKind.TS,
	);
	const value = new Map();
	const type = new Set();
	for (const stmt of source.statements) {
		if (ts.isClassDeclaration(stmt) && stmt.name) {
			value.set(stmt.name.text, "class");
		} else if (ts.isFunctionDeclaration(stmt) && stmt.name) {
			value.set(stmt.name.text, "function");
		} else if (ts.isVariableStatement(stmt)) {
			for (const decl of stmt.declarationList.declarations) {
				if (ts.isIdentifier(decl.name)) value.set(decl.name.text, "const");
			}
		} else if (ts.isInterfaceDeclaration(stmt)) {
			type.add(stmt.name.text);
		} else if (ts.isTypeAliasDeclaration(stmt)) {
			type.add(stmt.name.text);
		}
	}
	return { value, type };
}

function sorted(map) {
	return [...map.keys()].sort();
}

const declFiles = ["index.d.ts", "index.d.cts", "index.d.mts"].map((f) =>
	path.join(root, f),
);
const parsed = declFiles.map(parseDeclaration);
const valueSpaces = parsed.map((p) => sorted(p.value));
const typeSpaces = parsed.map((p) => [...p.type].sort());

for (let i = 1; i < parsed.length; i++) {
	if (JSON.stringify(valueSpaces[i]) !== JSON.stringify(valueSpaces[0])) {
		fail(
			`declaration value-space drift (${path.basename(declFiles[i])}): ${JSON.stringify(
				valueSpaces[i],
			)}`,
		);
	}
	if (JSON.stringify(typeSpaces[i]) !== JSON.stringify(typeSpaces[0])) {
		fail(
			`declaration type-space drift (${path.basename(declFiles[i])}): ${JSON.stringify(
				typeSpaces[i],
			)}`,
		);
	}
}

if (JSON.stringify(valueSpaces[0]) !== JSON.stringify(values)) {
	fail(
		`declared value space diverged: expected ${JSON.stringify(values)}, got ${JSON.stringify(
			valueSpaces[0],
		)}`,
	);
}
if (JSON.stringify(typeSpaces[0]) !== JSON.stringify(types)) {
	fail(
		`declared type space diverged: expected ${JSON.stringify(types)}, got ${JSON.stringify(
			typeSpaces[0],
		)}`,
	);
}

const cjs = require(path.join(root, "index.js"));
const cjsKeys = Object.keys(cjs).sort();
if (JSON.stringify(cjsKeys) !== JSON.stringify(values)) {
	fail(
		`CJS runtime keys diverged: expected ${JSON.stringify(values)}, got ${JSON.stringify(
			cjsKeys,
		)}`,
	);
}

const esm = await import(pathToFileURL(path.join(root, "index.mjs")).href);
const esmKeys = Object.keys(esm).sort();
if (JSON.stringify(esmKeys) !== JSON.stringify(values)) {
	fail(
		`ESM runtime keys diverged: expected ${JSON.stringify(values)}, got ${JSON.stringify(
			esmKeys,
		)}`,
	);
}

for (const name of classNames) {
	if (cjs[name] !== esm[name]) {
		fail(`class identity mismatch for ${name} (dual-package hazard)`);
	}
}

for (let i = 0; i < functions.length; i++) {
	const fn = functions[i];
	const cnst = constants[i];
	for (const [label, ns] of [
		["CJS", cjs],
		["ESM", esm],
	]) {
		if (typeof ns[fn] !== "function") {
			fail(`${label} ${fn} is not a function`);
		}
		if (typeof ns[cnst] !== "number") {
			fail(`${label} ${cnst} is not a number`);
		}
		if (ns[fn]() !== ns[cnst]) {
			fail(`${label} ${fn}() !== ${cnst}`);
		}
	}
}

console.log(
	"OK: 16 value + 4 type-only exports consistent across .d.ts/.d.cts/.d.mts, CJS, and ESM; class identity and constant equivalence hold.",
);
