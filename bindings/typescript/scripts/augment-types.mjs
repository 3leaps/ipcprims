import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { spawnSync } from "node:child_process";

const dir = path.dirname(fileURLToPath(import.meta.url));
const dtsPath = path.join(dir, "..", "index.d.ts");
let dts = readFileSync(dtsPath, "utf8");

const constants = `export declare const CONTROL: number
export declare const COMMAND: number
export declare const DATA: number
export declare const TELEMETRY: number
export declare const ERROR: number
`;

if (!dts.includes("export declare const COMMAND")) {
	dts = dts.replace(
		"export declare function error(): number\n",
		`export declare function error(): number\n${constants}`,
	);
}

dts = dts.replace(
	"export declare class AsyncChannelReceiver {\n  recvAsync(options?: RecvAsyncOptions | undefined | null): Promise<JsFrame>\n}",
	"export declare class AsyncChannelReceiver implements AsyncIterable<JsFrame> {\n  recvAsync(options?: RecvAsyncOptions | undefined | null): Promise<JsFrame>\n  [Symbol.asyncIterator](): AsyncIterator<JsFrame>\n}",
);

writeFileSync(dtsPath, dts);

const format = spawnSync(
	"goneat",
	["--log-level", "error", "format", "--files", dtsPath, "--quiet"],
	{
		stdio: "inherit",
	},
);

if (format.error && format.error.code !== "ENOENT") {
	throw format.error;
}

if (!format.error && format.status !== 0) {
	process.exit(format.status ?? 1);
}
