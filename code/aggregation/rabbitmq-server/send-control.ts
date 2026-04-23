import amqp from "amqplib";

type Args = {
  node: string;
  maxMessagesPerSecond?: number;
  maxMeasurementsPerMessage?: number;
  publishIntervalMs?: number;
};

function parseArgs(argv: string[]): Args {
  const args: Args = { node: "all" };

  for (let i = 0; i < argv.length; i += 1) {
    const current = argv[i];
    const next = argv[i + 1];

    if (current === "--node" && next) {
      args.node = next;
      i += 1;
    } else if (current === "--mps" && next) {
      args.maxMessagesPerSecond = Number(next);
      i += 1;
    } else if (current === "--max-per-message" && next) {
      args.maxMeasurementsPerMessage = Number(next);
      i += 1;
    } else if (current === "--interval-ms" && next) {
      args.publishIntervalMs = Number(next);
      i += 1;
    }
  }

  return args;
}

function printUsage(): void {
  console.log("Usage:");
  console.log(
    "bun run send-control.ts --node <node-id|all> --mps <n> --max-per-message <n> --interval-ms <n>",
  );
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function nextBackoffMs(attempt: number, minMs: number, maxMs: number): number {
  const exp = Math.min(maxMs, minMs * 2 ** Math.max(0, attempt - 1));
  const jitter = Math.floor(Math.random() * Math.max(1, Math.floor(exp * 0.2)));
  return exp + jitter;
}

async function main(): Promise<void> {
  const parsed = parseArgs(Bun.argv.slice(2));

  if (
    parsed.maxMessagesPerSecond === undefined &&
    parsed.maxMeasurementsPerMessage === undefined &&
    parsed.publishIntervalMs === undefined
  ) {
    printUsage();
    process.exit(1);
  }

  const amqpUrl = process.env.AMQP_URL ?? "amqp://uwb:uwb@localhost:5672";
  const controlExchange = process.env.CONTROL_EXCHANGE ?? "uwb.control";
  const errorExchange = process.env.ERROR_EXCHANGE ?? "uwb.errors";
  const retryCount = Number(process.env.SEND_CONTROL_RETRY_COUNT ?? "8");
  const retryMinMs = Number(process.env.SEND_CONTROL_RETRY_MIN_MS ?? "300");
  const retryMaxMs = Number(process.env.SEND_CONTROL_RETRY_MAX_MS ?? "5000");

  const payload = {
    maxMessagesPerSecond: parsed.maxMessagesPerSecond,
    maxMeasurementsPerMessage: parsed.maxMeasurementsPerMessage,
    publishIntervalMs: parsed.publishIntervalMs,
  };

  const routingKey = parsed.node === "all" ? "node.all" : `node.${parsed.node}`;

  for (let attempt = 1; attempt <= retryCount; attempt += 1) {
    let conn: any = null;
    let channel: any = null;
    let errorChannel: any = null;

    try {
      conn = await amqp.connect(amqpUrl);
      channel = await conn.createConfirmChannel();
      errorChannel = await conn.createChannel();

      await channel.assertExchange(controlExchange, "topic", { durable: true });
      await errorChannel.assertExchange(errorExchange, "topic", { durable: true });
      channel.publish(controlExchange, routingKey, Buffer.from(JSON.stringify(payload)), {
        contentType: "application/json",
        persistent: true,
        timestamp: Date.now(),
      });

      await channel.waitForConfirms();
      console.log("Sent control message", { routingKey, payload, attempt });
      await channel.close();
      await errorChannel.close();
      await conn.close();
      return;
    } catch (error) {
      console.error(`control publish failed (attempt ${String(attempt)}/${String(retryCount)})`, error);

      try {
        errorChannel?.publish(
          errorExchange,
          "control.send.failed",
          Buffer.from(
            JSON.stringify({
              component: "rabbitmq-server/send-control",
              type: "control-publish-failed",
              message: "failed to publish control message",
              recoverable: attempt < retryCount,
              details: {
                attempt,
                retryCount,
                routingKey,
              },
              crMS: Date.now(),
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

      try {
        await channel?.close();
      } catch {
        // ignore close errors
      }

      try {
        await errorChannel?.close();
      } catch {
        // ignore close errors
      }

      try {
        await conn?.close();
      } catch {
        // ignore close errors
      }

      if (attempt >= retryCount) {
        throw error;
      }

      const delayMs = nextBackoffMs(attempt, retryMinMs, retryMaxMs);
      await sleep(delayMs);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
