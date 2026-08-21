import { readdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { SerialPort } from "serialport";
import { ReadlineParser } from "@serialport/parser-readline";

type DeviceConfig = {
    pioenv: string;
    tagid?: number;
    anchorid?: number;
    [key: string]: unknown;
};

type ConfigFile = {
    id_to_config: Record<string, DeviceConfig>;
};

type UploadResult = {
    port: string;
    deviceId?: string;
    env?: string;
    status: "uploaded" | "skipped" | "failed";
    message: string;
};

const PORT_DIR = "/dev";
const PORT_REGEX = /^ttyUSB\d+$/;
const BAUD_RATE = 115200;
const GETINFO_COMMAND = "AT+GETINFO\n";
const SERIAL_TIMEOUT_MS = 5500;

const thisFile = fileURLToPath(import.meta.url);
const thisDir = dirname(thisFile);
const uploaderDir = thisDir;
const projectRoot = resolve(uploaderDir, "..");
const configPath = resolve(uploaderDir, "device_to_config.json");
const platformioIniPath = resolve(projectRoot, "platformio.ini");

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

async function loadConfig(): Promise<ConfigFile> {
    const raw = await readFile(configPath, "utf8");
    const parsed = JSON.parse(raw) as ConfigFile;

    if (!parsed.id_to_config || typeof parsed.id_to_config !== "object") {
        throw new Error(`Invalid config format in ${configPath}`);
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

function extractDeviceId(infoPayload: string): string {
    // Accept either "INFO=ID" or "INFO=ID,..." formats.
    const [firstToken] = infoPayload.split(",");
    return firstToken.trim();
}

async function queryDeviceInfo(portPath: string): Promise<string | null> {
    const port = new SerialPort({
        path: portPath,
        baudRate: BAUD_RATE,
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
        }, SERIAL_TIMEOUT_MS);

        parser.on("data", (line: string) => {
            const trimmed = line.trim();
            if (!trimmed.startsWith("INFO=")) {
                return;
            }

            const payload = trimmed.substring("INFO=".length);
            const deviceId = extractDeviceId(payload);
            if (!deviceId) {
                return;
            }

            void finalize(deviceId);
        });

        port.on("error", () => {
            void finalize(null);
        });

        port.open(async (err?: Error | null) => {
            console.log(`Opened port ${portPath} at ${BAUD_RATE} baud.`);
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

function leftPad2(value: number): string {
    return String(value).padStart(2, "0");
}

function resolveEnvName(
    deviceId: string,
    config: DeviceConfig,
    availableEnvs: Set<string>
): { env: string | null; tried: string[] } {
    const tried: string[] = [];
    const pushCandidate = (candidate: string | undefined): void => {
        if (!candidate) {
            return;
        }
        if (!tried.includes(candidate)) {
            tried.push(candidate);
        }
    };

    pushCandidate(config.pioenv);

    if (typeof config.tagid === "number") {
        pushCandidate(`${config.pioenv}${leftPad2(config.tagid)}`);
    }

    if (typeof config.anchorid === "number") {
        pushCandidate(`${config.pioenv}${leftPad2(config.anchorid)}`);
    }

    const idDigits = deviceId.match(/(\d+)$/)?.[1];
    if (idDigits) {
        pushCandidate(`${config.pioenv}${idDigits.padStart(2, "0")}`);
    }

    for (const candidate of tried) {
        if (availableEnvs.has(candidate)) {
            return { env: candidate, tried };
        }
    }

    return { env: null, tried };
}

async function uploadWithPlatformio(env: string, portPath: string): Promise<void> {
    await new Promise<void>((resolveRun, rejectRun) => {
        const child = spawn(
            "platformio",
            [
                "run",
                "--target",
                "upload",
                "--environment",
                env,
                "--upload-port",
                portPath,
            ],
            {
                cwd: projectRoot,
                stdio: "inherit",
            }
        );

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

async function main(): Promise<void> {
    const [config, availableEnvs, ports] = await Promise.all([
        loadConfig(),
        loadPlatformioEnvNames(),
        discoverUsbPorts(),
    ]);

    if (ports.length === 0) {
        console.log("No /dev/ttyUSBx devices found.");
        return;
    }

    console.log(`Discovered ${ports.length} serial device(s): ${ports.join(", ")}`);
    console.log("Starting identification and upload process...");

    const results: UploadResult[] = [];

    for (const portPath of ports) {
        console.log(`\n[${portPath}] Querying device info...`);

        const deviceId = await queryDeviceInfo(portPath);
        if (!deviceId) {
            const message = "No INFO response received.";
            console.log(`[${portPath}] ${message}`);
            results.push({ port: portPath, status: "skipped", message });
            continue;
        }

        console.log(`[${portPath}] Device ID: ${deviceId}`);

        const deviceConfig = config.id_to_config[deviceId];
        if (!deviceConfig) {
            const message = `No mapping found in device_to_config.json for ${deviceId}.`;
            console.log(`[${portPath}] ${message}`);
            results.push({
                port: portPath,
                deviceId,
                status: "skipped",
                message,
            });
            continue;
        }

        const resolved = resolveEnvName(deviceId, deviceConfig, availableEnvs);
        if (!resolved.env) {
            const message = `Could not resolve a PlatformIO environment. Tried: ${resolved.tried.join(
                ", "
            )}`;
            console.log(`[${portPath}] ${message}`);
            results.push({
                port: portPath,
                deviceId,
                status: "failed",
                message,
            });
            continue;
        }

        const env = resolved.env;
        console.log(`[${portPath}] Uploading env ${env}...`);

        try {
            await uploadWithPlatformio(env, portPath);
            const message = "Upload completed successfully.";
            results.push({
                port: portPath,
                deviceId,
                env,
                status: "uploaded",
                message,
            });
            console.log(`[${portPath}] ${message}`);
        } catch (error) {
            const message = `Upload failed: ${
                error instanceof Error ? error.message : String(error)
            }`;
            results.push({
                port: portPath,
                deviceId,
                env,
                status: "failed",
                message,
            });
            console.log(`[${portPath}] ${message}`);
        }
    }

    console.log("\n=== Summary ===");
    for (const result of results) {
        const detail = [
            `port=${result.port}`,
            `status=${result.status}`,
            result.deviceId ? `device=${result.deviceId}` : undefined,
            result.env ? `env=${result.env}` : undefined,
            `message=${result.message}`,
        ]
            .filter(Boolean)
            .join(" | ");

        console.log(detail);
    }

    const failed = results.some((r) => r.status === "failed");
    if (failed) {
        process.exitCode = 1;
    }
}

main().catch((error: unknown) => {
    console.error("Fatal error:", error);
    process.exitCode = 1;
});
