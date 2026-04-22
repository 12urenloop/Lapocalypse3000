import amqp, { type ConsumeMessage } from "amqplib";
import { appendFile, mkdir } from "node:fs/promises";
import path from "node:path";

const cfg = {
  amqpUrl: process.env.AMQP_URL ?? "amqp://uwb:uwb@localhost:5672",
  dataExchange: process.env.DATA_EXCHANGE ?? "uwb.data",
  dataQueue: process.env.DATA_QUEUE ?? "uwb.persist",
  dataBindingKey: process.env.DATA_BINDING_KEY ?? "node.*",
  errorExchange: process.env.ERROR_EXCHANGE ?? "uwb.errors",
  outputFile: process.env.OUTPUT_FILE ?? "./data/uwb-measurements.jsonl",
  latencyReportIntervalMs: Number(process.env.LATENCY_REPORT_INTERVAL_MS ?? "5000"),
  reconnectMinMs: Number(process.env.RECONNECT_MIN_MS ?? "500"),
  reconnectMaxMs: Number(process.env.RECONNECT_MAX_MS ?? "10000"),
};

let isShuttingDown = false;
let reconnects = 0;
let totalLatencySamples = 0;
let latencySamplesCurrentWindow: number[] = [];

function quantile(sortedValues: number[], q: number): number {
  if (sortedValues.length === 0) {
    return 0;
  }

  const index = Math.min(sortedValues.length - 1, Math.max(0, Math.floor(sortedValues.length * q)));
  return sortedValues[index];
}

function recordLatencySample(latencyMs: number): void {
  latencySamplesCurrentWindow.push(latencyMs);
  totalLatencySamples += 1;
}

function reportLatencyWindow(): void {
  const sampleCount = latencySamplesCurrentWindow.length;
  if (sampleCount === 0) {
    console.log("[latency] no samples in this window", {
      intervalMs: cfg.latencyReportIntervalMs,
      totalLatencySamples,
    });
    return;
  }

  const sorted = [...latencySamplesCurrentWindow].sort((a, b) => a - b);
  const sum = latencySamplesCurrentWindow.reduce((acc, value) => acc + value, 0);
  const avg = sum / sampleCount;
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  const p50 = quantile(sorted, 0.5);
  const p95 = quantile(sorted, 0.95);

  console.log("[latency] publisher->consumer", {
    intervalMs: cfg.latencyReportIntervalMs,
    sampleCount,
    totalLatencySamples,
    minMs: Number(min.toFixed(2)),
    avgMs: Number(avg.toFixed(2)),
    p50Ms: Number(p50.toFixed(2)),
    p95Ms: Number(p95.toFixed(2)),
    maxMs: Number(max.toFixed(2)),
  });

  latencySamplesCurrentWindow = [];
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function nextBackoffMs(attempt: number): number {
  const exp = Math.min(cfg.reconnectMaxMs, cfg.reconnectMinMs * 2 ** Math.max(0, attempt - 1));
  const jitter = Math.floor(Math.random() * Math.max(1, Math.floor(exp * 0.2)));
  return exp + jitter;
}

function reportError(
  errorChannel: any,
  type: string,
  message: string,
  recoverable: boolean,
  details?: Record<string, unknown>,
): void {
  if (!errorChannel) {
    return;
  }

  try {
    errorChannel.publish(
      cfg.errorExchange,
      `consumer.persist.${type}`,
      Buffer.from(
        JSON.stringify({
          component: "consumer-simple",
          type,
          message,
          recoverable,
          details,
          createdAtMs: Date.now(),
        }),
      ),
      {
        contentType: "application/json",
        persistent: true,
        timestamp: Date.now(),
      },
    );
  } catch {
    // best effort only
  }
}

async function persistLine(line: string): Promise<void> {
  const outputDir = path.dirname(cfg.outputFile);
  await mkdir(outputDir, { recursive: true });
  await appendFile(cfg.outputFile, `${line}\n`, "utf8");
}

async function main(): Promise<void> {
  let attempt = 0;
  const latencyReporter = setInterval(reportLatencyWindow, cfg.latencyReportIntervalMs);

  process.on("SIGINT", () => {
    isShuttingDown = true;
  });

  while (!isShuttingDown) {
    let conn: any = null;
    let channel: any = null;
    let errorChannel: any = null;
    let closedPromiseResolve: (() => void) | null = null;
    const closedPromise = new Promise<void>((resolve) => {
      closedPromiseResolve = resolve;
    });

    try {
      conn = await amqp.connect(cfg.amqpUrl);
      channel = await conn.createChannel();
      errorChannel = await conn.createChannel();

      await channel.assertExchange(cfg.dataExchange, "topic", { durable: true });
      await errorChannel.assertExchange(cfg.errorExchange, "topic", { durable: true });
      await channel.assertQueue(cfg.dataQueue, { durable: true });
      await channel.bindQueue(cfg.dataQueue, cfg.dataExchange, cfg.dataBindingKey);
      await channel.prefetch(100);

      attempt = 0;
      console.log("consumer connected", cfg);

      conn.on("close", () => {
        closedPromiseResolve?.();
      });
      conn.on("error", (error: unknown) => {
        console.error("consumer amqp error", error);
      });

      await channel.consume(cfg.dataQueue, (msg: ConsumeMessage | null) => {
        if (!msg) {
          return;
        }

        void (async () => {
          try {
            const consumedAtMs = Date.now();
            const raw = msg.content.toString();
            const parsed = JSON.parse(raw);
            const publisherTimestamp =
              typeof parsed?.publishedAtMs === "number"
                ? parsed.publishedAtMs
                : typeof parsed?.createdAtMs === "number"
                  ? parsed.createdAtMs
                  : undefined;

            if (typeof publisherTimestamp === "number") {
              const latencyMs = consumedAtMs - publisherTimestamp;
              recordLatencySample(latencyMs);
            }

            const line = JSON.stringify({
              persistedAtMs: consumedAtMs,
              routingKey: msg.fields.routingKey,
              payload: parsed,
            });

            await persistLine(line);
            channel.ack(msg);
          } catch (error) {
            if (error instanceof SyntaxError) {
              console.error("dropping invalid json payload", error);
              reportError(errorChannel, "invalid-json-payload", "dropping invalid JSON payload", false);
              channel.ack(msg);
              return;
            }

            console.error("failed to persist message, requeueing", error);
            reportError(errorChannel, "persist-failed", "failed to persist message, requeueing", true);
            channel.nack(msg, false, true);
          }
        })();
      });

      await closedPromise;
    } catch (error) {
      console.error("consumer connection loop error", error);
    } finally {
      try {
        await errorChannel?.close();
      } catch {
        // ignore close errors during reconnect
      }

      try {
        await channel?.close();
      } catch {
        // ignore close errors during reconnect
      }

      try {
        await conn?.close();
      } catch {
        // ignore close errors during reconnect
      }
    }

    if (isShuttingDown) {
      break;
    }

    reconnects += 1;
    attempt += 1;
    const delayMs = nextBackoffMs(attempt);
    console.log(`consumer reconnecting in ${String(delayMs)}ms`, { reconnects });
    await sleep(delayMs);
  }

  clearInterval(latencyReporter);
  reportLatencyWindow();

  process.exit(0);
}

main().catch((error) => {
  console.error("consumer failed", error);
  process.exit(1);
});
