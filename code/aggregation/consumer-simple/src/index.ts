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
  reconnectMinMs: Number(process.env.RECONNECT_MIN_MS ?? "500"),
  reconnectMaxMs: Number(process.env.RECONNECT_MAX_MS ?? "10000"),
};

let isShuttingDown = false;
let reconnects = 0;

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
            const raw = msg.content.toString();
            const parsed = JSON.parse(raw);
            const line = JSON.stringify({
              persistedAtMs: Date.now(),
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

  process.exit(0);
}

main().catch((error) => {
  console.error("consumer failed", error);
  process.exit(1);
});
