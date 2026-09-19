import { readdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { SerialPort } from "serialport";
import { ReadlineParser } from "@serialport/parser-readline";

type TypeConfig = {
    pioenv: string;
    id_var: string;
    extra_build_flags?: string[];
};

type ConfigFile = {
    type_to_config: Record<string, TypeConfig>;
    default_upload?: { type: string; id: number };
    baud_rate?: number;
    serial_timeout_ms?: number;
};

type ParsedInfo = {
    raw: string;
    type?: string;
    id?: number;
};

type UploadResult = {
    port: string;
    deviceInfo?: string;
    deviceType?: string;
    deviceId?: number;
    env?: string;
    status: "uploaded" | "skipped" | "failed";
    message: string;
};

type CliOptions = {
    port?: string;
    type?: string;
    id?: number;
    pioenv?: string;
    idVar?: string;
    infostring?: string;
    configPath?: string;
    dryRun: boolean;
    noVerify: boolean;
    help: boolean;
};

const PORT_DIR = "/dev";
const PORT_REGEX = /^ttyUSB\d+$/;
const DEFAULT_BAUD_RATE = 115200;
const GETINFO_COMMAND = "AT+GETINFO\n";
const DEFAULT_SERIAL_TIMEOUT_MS = 5500;

const thisFile = fileURLToPath(import.meta.url);
const thisDir = dirname(thisFile);
const uploaderDir = thisDir;
const projectRoot = resolve(uploaderDir, "..");
const defaultConfigPath = resolve(uploaderDir, "device_to_config.json");
const platformioIniPath = resolve(projectRoot, "platformio.ini");

function printHelp(): void {
    console.log(`Auto-select, configure and upload firmware via PlatformIO.

Usage:
  auto-upload                                    Scan /dev/ttyUSB*, query AT+GETINFO,
                                                 resolve type->pioenv via config and upload.
  auto-upload --port /dev/ttyUSB0                Query single port, resolve and upload.
  auto-upload --port <port> --type <t> --id <n>  Provision chosen port as type/id from config.
  auto-upload --port <port> --pioenv <env>
      [--id-var <VAR> --id <n>] [--infostring s] Upload arbitrary pioenv to chosen port.

Options:
  --port <path>       Target serial port (manual single-port mode).
  --type <type>       Device type key from type_to_config in the config file.
  --id <number>       Numeric device id, programmed into <id_var> and INFOSTRING.
  --pioenv <env>      Arbitrary PlatformIO environment (bypasses type lookup).
                      INFOSTRING comes from --infostring, is built from
                      --type/--id, or is carried over from the device's
                      current AT+GETINFO answer.
  --id-var <VAR>      C define name for the id (used with --pioenv --id).
  --infostring <s>    Override INFOSTRING content (default: "type=<type>,id=<id>").
  --config <path>     Config file (default: new_device_to_config.json).
  --dry-run           Print what would be uploaded without running platformio.
  --no-verify         Skip the post-upload AT+GETINFO verification.
  --help              Show this help.

Config (${defaultConfigPath}):
  {
    "type_to_config": {
      "anchor": { "pioenv": "wemosanchor", "id_var": "ANCHOR_ID" },
      "tag":    { "pioenv": "nrftag",      "id_var": "TAG_ID" }
    },
    "default_upload": { "type": "anchor", "id": 1 }   // optional fallback for
  }                                                   // blank/unknown devices

Each upload programs two defines on top of platformio.ini build flags:
  -D<id_var>=<id>  and  -DINFOSTRING="\\"type=<type>,id=<id>\\""
so AT+GETINFO answers e.g.  INFO=type=anchor,id=1.
After each real upload the device is re-queried and the reported INFO is
compared against the expected INFOSTRING (skip with --no-verify).
Ports with no/unknown INFO use default_upload when set, otherwise fail
with a hint to provision them via --port --type --id.`);
}

function parseArgs(argv: string[]): CliOptions {
    const opts: CliOptions = { dryRun: false, noVerify: false, help: false };
    for (let i = 0; i < argv.length; i++) {
        const arg = argv[i];
        const next = argv[i + 1];
        const needValue = (name: string): string => {
            if (next === undefined || next.startsWith("--")) {
                throw new Error(`Missing value for ${name}.`);
            }
            i++;
            return next;
        };
        switch (arg) {
            case "--help":
            case "-h":
                opts.help = true;
                break;
            case "--port":
                opts.port = needValue("--port");
                break;
            case "--type":
                opts.type = needValue("--type");
                break;
            case "--id":
                opts.id = Number.parseInt(needValue("--id"), 10);
                break;
            case "--pioenv":
                opts.pioenv = needValue("--pioenv");
                break;
            case "--id-var":
                opts.idVar = needValue("--id-var");
                break;
            case "--infostring":
                opts.infostring = needValue("--infostring");
                break;
            case "--config":
                opts.configPath = resolve(needValue("--config"));
                break;
            case "--dry-run":
                opts.dryRun = true;
                break;
            case "--no-verify":
                opts.noVerify = true;
                break;
            default:
                throw new Error(`Unknown argument: ${arg}. Use --help.`);
        }
    }
    return opts;
}

function sortUsbPorts(a: string, b: string): number {
    const aNum = Number.parseInt(a.replace("ttyUSB", ""), 10);
    const bNum = Number.parseInt(b.replace("ttyUSB", ""), 10);
    return aNum - bNum;
}

async function discoverUsbPorts(): Promise<string[]> {
    const names = await readdir(PORT_DIR);
    const ports = names
        .filter((name: string) => PORT_REGEX.test(name))
        .sort(sortUsbPorts);
    return ports.map((name: string) => `${PORT_DIR}/${name}`);
}

async function loadConfig(configPath: string): Promise<ConfigFile> {
    const raw = await readFile(configPath, "utf8");
    const parsed = JSON.parse(raw) as ConfigFile;

    if (!parsed.type_to_config || typeof parsed.type_to_config !== "object") {
        throw new Error(
            `Invalid config in ${configPath}: missing "type_to_config" object.`
        );
    }
    for (const [type, entry] of Object.entries(parsed.type_to_config)) {
        if (!entry || typeof entry.pioenv !== "string" || !entry.pioenv) {
            throw new Error(`Invalid config: type "${type}" has no pioenv.`);
        }
        if (typeof entry.id_var !== "string" || !entry.id_var) {
            throw new Error(`Invalid config: type "${type}" has no id_var.`);
        }
    }
    if (parsed.default_upload !== undefined) {
        const d = parsed.default_upload;
        if (
            typeof d.type !== "string" ||
            !parsed.type_to_config[d.type] ||
            !Number.isInteger(d.id)
        ) {
            throw new Error(
                `Invalid config: default_upload must reference a known type with an integer id.`
            );
        }
    }
    return parsed;
}

async function loadPlatformioEnvNames(): Promise<Set<string>> {
    const content = await readFile(platformioIniPath, "utf8");
    const envSet = new Set<string>();

    const envRegex = /^\[env:([^\]]+)\]$/gm;
    let match: RegExpExecArray | null = envRegex.exec(content);

    while (match) {
        envSet.add(match[1].trim());
        match = envRegex.exec(content);
    }

    return envSet;
}

function stripQuotes(value: string): string {
    const trimmed = value.trim();
    if (
        trimmed.length >= 2 &&
        trimmed.startsWith('"') &&
        trimmed.endsWith('"')
    ) {
        return trimmed.slice(1, -1);
    }
    return trimmed;
}

function parseInfoPayload(payload: string): ParsedInfo {
    const raw = payload.trim();
    const result: ParsedInfo = { raw };
    if (!raw || !raw.includes("=")) {
        return result;
    }
    // Expected form: "type=anchor,id=1" (order-independent, extra keys ignored).
    for (const token of raw.split(",")) {
        const eq = token.indexOf("=");
        if (eq === -1) {
            continue;
        }
        const key = token.slice(0, eq).trim().toLowerCase();
        const value = stripQuotes(token.slice(eq + 1));
        if (key === "type" && value) {
            result.type = value;
        } else if (key === "id" && value) {
            const id = Number.parseInt(value, 10);
            if (Number.isInteger(id)) {
                result.id = id;
            }
        }
    }
    return result;
}

function buildInfoString(type: string, id: number): string {
    return `type=${type},id=${id}`;
}

function buildExtraFlags(args: {
    idVar?: string;
    id?: number;
    infostring?: string;
    extra?: string[];
}): string[] {
    const flags: string[] = [];
    if (args.idVar && args.id !== undefined) {
        if (!Number.isInteger(args.id)) {
            throw new Error(`Device id must be an integer, got: ${args.id}`);
        }
        flags.push(`-D${args.idVar}=${args.id}`);
    }
    if (args.infostring !== undefined) {
        const clean = args.infostring.replace(/"/g, "");
        flags.push(`-DINFOSTRING="\\"${clean}\\""`);
    }
    if (args.extra) {
        flags.push(...args.extra);
    }
    return flags;
}

async function queryDeviceInfo(
    portPath: string,
    baudRate: number,
    timeoutMs: number
): Promise<string | null> {
    const port = new SerialPort({
        path: portPath,
        baudRate,
        autoOpen: false,
    });

    const closePort = async (): Promise<void> => {
        if (!port.isOpen) {
            return;
        }

        await new Promise<void>((resolveClose) => {
            port.close(() => resolveClose());
        });
    };

    return new Promise<string | null>((resolveResult) => {
        let settled = false;

        const finalize = async (value: string | null): Promise<void> => {
            if (settled) {
                return;
            }
            settled = true;
            clearTimeout(timeout);
            await closePort();
            resolveResult(value);
        };

        const parser = port.pipe(
            new ReadlineParser({
                delimiter: "\n",
                encoding: "ascii",
            })
        );

        const timeout = setTimeout(() => {
            void finalize(null);
        }, timeoutMs);

        parser.on("data", (line: string) => {
            const trimmed = line.trim();
            if (!trimmed.startsWith("INFO=")) {
                return;
            }

            const payload = trimmed.substring("INFO=".length).trim();
            if (!payload) {
                return;
            }

            void finalize(payload);
        });

        port.on("error", () => {
            void finalize(null);
        });

        port.open(async (err?: Error | null) => {
            console.log(`Opened port ${portPath} at ${baudRate} baud.`);
            if (err) {
                console.error(`Failed to open port ${portPath}:`, err);
                void finalize(null);
                return;
            }

            await new Promise((resolve) => setTimeout(resolve, 2000));

            port.write(GETINFO_COMMAND, (writeErr?: Error | null) => {
                if (writeErr) {
                    void finalize(null);
                }
            });
        });
    });
}

const VERIFY_REBOOT_WAIT_MS = 7000;
const VERIFY_ATTEMPTS = 3;

async function verifyInfostring(args: {
    port: string;
    expected: string;
    baudRate: number;
    timeoutMs: number;
}): Promise<{ ok: true; got: string } | { ok: false; got: string | null }> {
    console.log(
        `[${args.port}] Waiting for reboot, then verifying reported INFO...`
    );
    await new Promise((resolve) => setTimeout(resolve, VERIFY_REBOOT_WAIT_MS));
    const expectedParsed = parseInfoPayload(args.expected);
    for (let attempt = 1; attempt <= VERIFY_ATTEMPTS; attempt++) {
        const payload = await queryDeviceInfo(
            args.port,
            args.baudRate,
            args.timeoutMs
        );
        if (payload === null) {
            console.log(
                `[${args.port}] Verify attempt ${attempt}/${VERIFY_ATTEMPTS}: no INFO response, retrying...`
            );
            continue;
        }
        const parsed = parseInfoPayload(payload);
        const match =
            payload === args.expected ||
            (expectedParsed.type !== undefined &&
                parsed.type === expectedParsed.type &&
                expectedParsed.id !== undefined &&
                parsed.id === expectedParsed.id);
        return { ok: match, got: payload };
    }
    return { ok: false, got: null };
}

async function uploadWithPlatformio(
    env: string,
    portPath: string,
    buildFlags: string[],
    dryRun: boolean
): Promise<void> {
    const pioArgs = [
        "run",
        "--target",
        "upload",
        "--environment",
        env,
        "--upload-port",
        portPath,
    ];
    const prettyFlags = buildFlags.join(" ");
    console.log(`[upload] platformio ${pioArgs.join(" ")}`);
    if (prettyFlags) {
        console.log(`[upload] PLATFORMIO_BUILD_FLAGS=${prettyFlags}`);
    }
    if (dryRun) {
        console.log("[upload] dry-run: skipping platformio invocation.");
        return;
    }
    await new Promise<void>((resolveRun, rejectRun) => {
        const child = spawn("platformio", pioArgs, {
            cwd: projectRoot,
            stdio: "inherit",
            env: {
                ...process.env,
                ...(prettyFlags
                    ? { PLATFORMIO_BUILD_FLAGS: prettyFlags }
                    : {}),
            },
        });

        child.on("error", (err) => {
            rejectRun(err);
        });

        child.on("close", (code: number | null) => {
            if (code === 0) {
                resolveRun();
                return;
            }
            rejectRun(new Error(`platformio exited with code ${code}`));
        });
    });
}

function requireEnvExists(env: string, availableEnvs: Set<string>): void {
    if (!availableEnvs.has(env)) {
        throw new Error(
            `PlatformIO environment "${env}" not found in platformio.ini. ` +
                `Available: ${[...availableEnvs].join(", ") || "(none)"}`
        );
    }
}

async function performUpload(args: {
    port: string;
    type: string;
    id: number;
    config: ConfigFile;
    availableEnvs: Set<string>;
    baudRate: number;
    timeoutMs: number;
    dryRun: boolean;
    noVerify: boolean;
    infostringOverride?: string;
    origin: string;
}): Promise<UploadResult> {
    const typeConfig = args.config.type_to_config[args.type];
    if (!typeConfig) {
        return {
            port: args.port,
            status: "failed",
            message:
                `Unknown type "${args.type}" (${args.origin}). ` +
                `Known types: ${Object.keys(args.config.type_to_config).join(", ")}.`,
        };
    }
    try {
        requireEnvExists(typeConfig.pioenv, args.availableEnvs);
        const infostring =
            args.infostringOverride ?? buildInfoString(args.type, args.id);
        const flags = buildExtraFlags({
            idVar: typeConfig.id_var,
            id: args.id,
            infostring,
            extra: typeConfig.extra_build_flags,
        });
        console.log(
            `[${args.port}] ${args.origin} -> type=${args.type} id=${args.id} ` +
                `env=${typeConfig.pioenv} ${typeConfig.id_var}=${args.id} ` +
                `INFOSTRING="${infostring}"`
        );
        await uploadWithPlatformio(
            typeConfig.pioenv,
            args.port,
            flags,
            args.dryRun
        );
        if (args.dryRun) {
            const message = "Dry run completed successfully.";
            console.log(`[${args.port}] ${message}`);
            return {
                port: args.port,
                deviceInfo: infostring,
                deviceType: args.type,
                deviceId: args.id,
                env: typeConfig.pioenv,
                status: "uploaded",
                message,
            };
        }
        if (!args.noVerify) {
            const verification = await verifyInfostring({
                port: args.port,
                expected: infostring,
                baudRate: args.baudRate,
                timeoutMs: args.timeoutMs,
            });
            if (!verification.ok) {
                const failMessage =
                    verification.got === null
                        ? `Upload succeeded but device did not answer AT+GETINFO after reboot (expected INFO=${infostring}).`
                        : `Upload succeeded but device reports INFO=${verification.got} (expected INFO=${infostring}). Old firmware may still be running or the wrong port was flashed.`;
                console.log(`[${args.port}] ${failMessage}`);
                return {
                    port: args.port,
                    deviceInfo: verification.got ?? infostring,
                    deviceType: args.type,
                    deviceId: args.id,
                    env: typeConfig.pioenv,
                    status: "failed",
                    message: failMessage,
                };
            }
            const message = `Upload completed and verified: device reports INFO=${verification.got}.`;
            console.log(`[${args.port}] ${message}`);
            return {
                port: args.port,
                deviceInfo: verification.got,
                deviceType: args.type,
                deviceId: args.id,
                env: typeConfig.pioenv,
                status: "uploaded",
                message,
            };
        }
        const message = "Upload completed successfully (verification skipped).";
        console.log(`[${args.port}] ${message}`);
        return {
            port: args.port,
            deviceInfo: infostring,
            deviceType: args.type,
            deviceId: args.id,
            env: typeConfig.pioenv,
            status: "uploaded",
            message,
        };
    } catch (error) {
        const message = `Upload failed: ${
            error instanceof Error ? error.message : String(error)
        }`;
        console.log(`[${args.port}] ${message}`);
        return {
            port: args.port,
            deviceType: args.type,
            deviceId: args.id,
            env: typeConfig.pioenv,
            status: "failed",
            message,
        };
    }
}

function resolveAutoTarget(
    parsed: ParsedInfo,
    config: ConfigFile
): { type: string; id: number; origin: string } | { error: string } {
    if (
        parsed.type !== undefined &&
        parsed.id !== undefined &&
        config.type_to_config[parsed.type]
    ) {
        return {
            type: parsed.type,
            id: parsed.id,
            origin: `identified as "${parsed.raw}"`,
        };
    }
    if (config.default_upload) {
        const reason = !parsed.raw
            ? "empty INFO response"
            : `unrecognized INFO "${parsed.raw}"`;
        return {
            type: config.default_upload.type,
            id: config.default_upload.id,
            origin: `using default_upload (${reason})`,
        };
    }
    return {
        error:
            `Cannot identify device (INFO="${parsed.raw || "(none)"}"). ` +
            `No default_upload set in config. Provision it explicitly, e.g.: ` +
            `--port <port> --type <type> --id <n>.`,
    };
}

async function handleAutoPort(
    portPath: string,
    config: ConfigFile,
    availableEnvs: Set<string>,
    baudRate: number,
    timeoutMs: number,
    dryRun: boolean,
    noVerify: boolean
): Promise<UploadResult> {
    console.log(`\n[${portPath}] Querying device info...`);
    const payload = await queryDeviceInfo(portPath, baudRate, timeoutMs);
    if (!payload) {
        if (config.default_upload) {
            console.log(
                `[${portPath}] No INFO response, using default_upload.`
            );
            return performUpload({
                port: portPath,
                type: config.default_upload.type,
                id: config.default_upload.id,
                config,
                availableEnvs,
                baudRate,
                timeoutMs,
                dryRun,
                noVerify,
                origin: "no INFO response, using default_upload",
            });
        }
        const message =
            "No INFO response received and no default_upload set in config. " +
            "Provision explicitly, e.g.: --port <port> --type <type> --id <n>.";
        console.log(`[${portPath}] ${message}`);
        return { port: portPath, status: "failed", message };
    }

    console.log(`[${portPath}] INFO=${payload}`);
    const parsed = parseInfoPayload(payload);
    const target = resolveAutoTarget(parsed, config);
    if ("error" in target) {
        console.log(`[${portPath}] ${target.error}`);
        return {
            port: portPath,
            deviceInfo: payload,
            status: "failed",
            message: target.error,
        };
    }
    return performUpload({
        port: portPath,
        type: target.type,
        id: target.id,
        config,
        availableEnvs,
        baudRate,
        timeoutMs,
        dryRun,
        noVerify,
        origin: target.origin,
    });
}

async function handleManualPort(
    opts: CliOptions,
    config: ConfigFile,
    availableEnvs: Set<string>,
    baudRate: number,
    timeoutMs: number
): Promise<UploadResult[]> {
    const port = opts.port as string;

    // Fully arbitrary mode: --pioenv bypasses the type lookup.
    if (opts.pioenv) {
        requireEnvExists(opts.pioenv, availableEnvs);
        let infostring = opts.infostring;
        let infoOrigin = "explicit --infostring";
        if (infostring === undefined && opts.type !== undefined && opts.id !== undefined) {
            infostring = buildInfoString(opts.type, opts.id);
            infoOrigin = "built from --type/--id";
        }
        if (infostring === undefined && !opts.dryRun) {
            console.log(
                `[${port}] No --infostring/--type given, reading current INFO from device to carry it over...`
            );
            const current = await queryDeviceInfo(port, baudRate, timeoutMs);
            if (current === null) {
                throw new Error(
                    `Device on ${port} did not answer AT+GETINFO and no --infostring (or --type/--id) was given, ` +
                        `so INFOSTRING cannot be determined. Pass --infostring "type=<type>,id=<id>" explicitly.`
                );
            }
            infostring = current;
            infoOrigin = "carried over from device";
        }
        const flags = buildExtraFlags({
            idVar: opts.idVar,
            id: opts.id,
            infostring,
        });
        const detail =
            `manual upload env=${opts.pioenv}` +
            (opts.idVar && opts.id !== undefined
                ? ` ${opts.idVar}=${opts.id}`
                : "") +
            (infostring
                ? ` INFOSTRING="${infostring}" (${infoOrigin})`
                : ` INFOSTRING=<would carry over from device>`);
        console.log(`[${port}] ${detail}`);
        try {
            await uploadWithPlatformio(opts.pioenv, port, flags, opts.dryRun);
            if (!opts.dryRun && !opts.noVerify && infostring !== undefined) {
                const verification = await verifyInfostring({
                    port,
                    expected: infostring,
                    baudRate,
                    timeoutMs,
                });
                if (!verification.ok) {
                    const failMessage =
                        verification.got === null
                            ? `Upload succeeded but device did not answer AT+GETINFO after reboot (expected INFO=${infostring}).`
                            : `Upload succeeded but device reports INFO=${verification.got} (expected INFO=${infostring}).`;
                    return [
                        {
                            port,
                            env: opts.pioenv,
                            deviceInfo: verification.got ?? infostring,
                            status: "failed",
                            message: failMessage,
                        },
                    ];
                }
                const message = `Upload completed and verified: device reports INFO=${verification.got}.`;
                return [
                    {
                        port,
                        env: opts.pioenv,
                        deviceInfo: verification.got,
                        status: "uploaded",
                        message,
                    },
                ];
            }
            const message = opts.dryRun
                ? "Dry run completed successfully."
                : "Upload completed successfully (verification skipped).";
            return [
                {
                    port,
                    env: opts.pioenv,
                    deviceInfo: infostring,
                    status: "uploaded",
                    message,
                },
            ];
        } catch (error) {
            return [
                {
                    port,
                    env: opts.pioenv,
                    deviceInfo: infostring,
                    status: "failed",
                    message: `Upload failed: ${
                        error instanceof Error ? error.message : String(error)
                    }`,
                },
            ];
        }
    }

    // Typed manual mode: --type + --id resolve pioenv/id_var from config.
    if (opts.type !== undefined || opts.id !== undefined) {
        if (opts.type === undefined || opts.id === undefined) {
            throw new Error("Manual mode needs both --type <type> and --id <n>.");
        }
        if (!Number.isInteger(opts.id)) {
            throw new Error(`--id must be an integer, got: ${opts.id}`);
        }
        return [
            await performUpload({
                port,
                type: opts.type,
                id: opts.id,
                config,
                availableEnvs,
                baudRate,
                timeoutMs,
                dryRun: opts.dryRun,
                noVerify: opts.noVerify,
                infostringOverride: opts.infostring,
                origin: "manual --type/--id",
            }),
        ];
    }

    // Single-port auto mode: query the device, then resolve like auto.
    return [
        await handleAutoPort(
            port,
            config,
            availableEnvs,
            baudRate,
            timeoutMs,
            opts.dryRun,
            opts.noVerify
        ),
    ];
}

function printSummary(results: UploadResult[]): void {
    console.log("\n=== Summary ===");
    for (const result of results) {
        const detail = [
            `port=${result.port}`,
            `status=${result.status}`,
            result.deviceInfo ? `info=${result.deviceInfo}` : undefined,
            result.deviceType ? `type=${result.deviceType}` : undefined,
            result.deviceId !== undefined ? `id=${result.deviceId}` : undefined,
            result.env ? `env=${result.env}` : undefined,
            `message=${result.message}`,
        ]
            .filter(Boolean)
            .join(" | ");

        console.log(detail);
    }
}

async function main(): Promise<void> {
    const opts = parseArgs(process.argv.slice(2));
    if (opts.help) {
        printHelp();
        return;
    }
    if (opts.id !== undefined && !Number.isInteger(opts.id)) {
        throw new Error(`--id must be an integer, got: ${opts.id}`);
    }
    if (
        opts.port === undefined &&
        (opts.type !== undefined ||
            opts.id !== undefined ||
            opts.pioenv !== undefined ||
            opts.idVar !== undefined ||
            opts.infostring !== undefined)
    ) {
        throw new Error(
            "--type/--id/--pioenv/--id-var/--infostring require --port <path>."
        );
    }
    if (opts.idVar !== undefined && opts.id === undefined) {
        throw new Error("--id-var <VAR> needs --id <n> to go with it.");
    }
    if (
        opts.id !== undefined &&
        opts.idVar === undefined &&
        opts.type === undefined
    ) {
        throw new Error(
            "--id <n> needs --id-var <VAR> (e.g. --id-var ANCHOR_ID) or " +
                "--type <type> to resolve the define name from the config."
        );
    }
    if (opts.type !== undefined && opts.id === undefined) {
        throw new Error("--type <type> needs --id <n> to go with it.");
    }

    const configPath = opts.configPath ?? defaultConfigPath;
    const [config, availableEnvs] = await Promise.all([
        loadConfig(configPath),
        loadPlatformioEnvNames(),
    ]);
    const baudRate = config.baud_rate ?? DEFAULT_BAUD_RATE;
    const timeoutMs = config.serial_timeout_ms ?? DEFAULT_SERIAL_TIMEOUT_MS;

    console.log(`Using config: ${configPath}`);
    console.log(
        `Known types: ${Object.entries(config.type_to_config)
            .map(([t, c]) => `${t}->${c.pioenv} (${c.id_var})`)
            .join(", ")}`
    );
    if (config.default_upload) {
        console.log(
            `Default upload: type=${config.default_upload.type} id=${config.default_upload.id}`
        );
    }

    let results: UploadResult[];
    if (opts.port) {
        results = await handleManualPort(
            opts,
            config,
            availableEnvs,
            baudRate,
            timeoutMs
        );
    } else {
        const ports = await discoverUsbPorts();
        if (ports.length === 0) {
            console.log("No /dev/ttyUSBx devices found.");
            return;
        }
        console.log(
            `Discovered ${ports.length} serial device(s): ${ports.join(", ")}`
        );
        console.log("Starting identification and upload process...");
        results = [];
        for (const portPath of ports) {
            results.push(
                await handleAutoPort(
                    portPath,
                    config,
                    availableEnvs,
                    baudRate,
                    timeoutMs,
                    opts.dryRun,
                    opts.noVerify
                )
            );
        }
    }

    printSummary(results);

    const failed = results.some((r) => r.status === "failed");
    if (failed) {
        process.exitCode = 1;
    }
}

main().catch((error: unknown) => {
    console.error("Fatal error:", error);
    process.exitCode = 1;
});
