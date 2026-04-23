import amqp, { type Channel, type Connection, type ConsumeMessage } from "amqplib";
import { $ } from "bun";

type Measurement = {
  tagId: string;
  distanceCm: number;
  syncedTimeMs: number;
  unsyncedMs: number;
  receivedAtMs: number;
};

type ControlMessage = {
  maxMessagesPerSecond?: number;
  maxMeasurementsPerMessage?: number;
  publishIntervalMs?: number;
};

type ErrorEvent = {
  component: string;
  nodeId: string;
  type: string;
  message: string;
  recoverable: boolean;
  details?: Record<string, unknown>;
  crMS: number;
};

const cfg = {
  nodeId: process.env.NODE_ID ?? "pi-01",
  serialDevice: process.env.SERIAL_DEVICE ?? "/dev/ttyUSB0",
  serialBaud: Number(process.env.SERIAL_BAUD ?? "115200"),
  amqpUrl: process.env.AMQP_URL ?? "amqp://uwb:uwb@localhost:5672",
  dataExchange: process.env.DATA_EXCHANGE ?? "uwb.data",
  controlExchange: process.env.CONTROL_EXCHANGE ?? "uwb.control",
  errorExchange: process.env.ERROR_EXCHANGE ?? "uwb.errors",
  maxPendingMeasurements: Number(process.env.MAX_PENDING_MEASUREMENTS ?? "5000"),
  reconnectMinMs: Number(process.env.RECONNECT_MIN_MS ?? "500"),
  reconnectMaxMs: Number(process.env.RECONNECT_MAX_MS ?? "10000"),
};

const limits = {
  maxMessagesPerSecond: Number(process.env.MAX_MESSAGES_PER_SECOND ?? "5"),
  maxMeasurementsPerMessage: Number(process.env.MAX_MEASUREMENTS_PER_MESSAGE ?? "25"),
  publishIntervalMs: Number(process.env.PUBLISH_INTERVAL_MS ?? "200"),
};

let pending: Measurement[] = [];
let droppedMeasurements = 0;
let skippedFlushesForRateLimit = 0;
let sentMessages = 0;
let bseq = 0;
let windowSecond = Math.floor(Date.now() / 1000);
let sentThisWindow = 0;
let serialReconnects = 0;
let amqpReconnects = 0;
let isShuttingDown = false;

let conn: Connection | null = null;
let channel: Channel | null = null;
let errorChannel: Channel | null = null;
let flushTimer: Timer | null = null;

const lineRegex = /^\s*=\s+([^\s]+)\s+(\d+)\s+mesh\s+(\d+)\s+(\d+)\s*$/;

function parseMeasurement(line: string): Measurement | null {
  const match = lineRegex.exec(line);
  if (!match) {
    return null;
  }

  return {
    tagId: match[1],
    distanceCm: Number(match[2]),
    syncedTimeMs: Number(match[3]),
    unsyncedMs: Number(match[4]),
    receivedAtMs: Date.now(),
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function nextBackoffMs(attempt: number): number {
  const exp = Math.min(cfg.reconnectMaxMs, cfg.reconnectMinMs * 2 ** Math.max(0, attempt - 1));
  const jitter = Math.floor(Math.random() * Math.max(1, Math.floor(exp * 0.2)));
  return exp + jitter;
}

function pushMeasurement(measurement: Measurement): void {
  pending.push(measurement);

  if (pending.length > cfg.maxPendingMeasurements) {
    const overflow = pending.length - cfg.maxPendingMeasurements;
    pending.splice(0, overflow);
    droppedMeasurements += overflow;
  }
}

function publishErrorEvent(event: ErrorEvent): void {
  if (!errorChannel) {
    return;
  }

  try {
    errorChannel.publish(
      cfg.errorExchange,
      `publisher.${cfg.nodeId}.${event.type}`,
      Buffer.from(JSON.stringify(event)),
      {
        contentType: "application/json",
        persistent: true,
        timestamp: Date.now(),
      },
    );
  } catch {
    // best effort only; avoid crash loops while reporting errors
  }
}

function reportError(
  type: string,
  message: string,
  recoverable: boolean,
  details?: Record<string, unknown>,
): void {
  publishErrorEvent({
    component: "publisher-bun",
    nodeId: cfg.nodeId,
    type,
    message,
    recoverable,
    details,
    crMS: Date.now(),
  });
}

function resetRateWindowIfNeeded(): void {
  const nowSecond = Math.floor(Date.now() / 1000);
  if (nowSecond !== windowSecond) {
    windowSecond = nowSecond;
    sentThisWindow = 0;
  }
}

function setFlushTimer(): void {
  if (flushTimer) {
    clearInterval(flushTimer);
  }

  flushTimer = setInterval(() => {
    void flushMeasurements();
  }, limits.publishIntervalMs);
}

function applyControl(update: ControlMessage): void {
  let changedInterval = false;

  if (typeof update.maxMessagesPerSecond === "number" && update.maxMessagesPerSecond > 0) {
    limits.maxMessagesPerSecond = Math.floor(update.maxMessagesPerSecond);
  }

  if (
    typeof update.maxMeasurementsPerMessage === "number" &&
    update.maxMeasurementsPerMessage > 0
  ) {
    limits.maxMeasurementsPerMessage = Math.floor(update.maxMeasurementsPerMessage);
  }

  if (typeof update.publishIntervalMs === "number" && update.publishIntervalMs > 0) {
    limits.publishIntervalMs = Math.floor(update.publishIntervalMs);
    changedInterval = true;
  }

  if (changedInterval) {
    setFlushTimer();
  }

  console.log("[control] active limits", limits);
}

async function flushMeasurements(): Promise<void> {
  if (!channel || pending.length === 0) {
    return;
  }

  resetRateWindowIfNeeded();

  if (sentThisWindow >= limits.maxMessagesPerSecond) {
    skippedFlushesForRateLimit += 1;
    return;
  }

  let batch = pending;
  pending = [];

  if (batch.length > limits.maxMeasurementsPerMessage) {
    const keep = batch.slice(0, limits.maxMeasurementsPerMessage);
    const discarded = batch.length - keep.length;
    batch = keep;
    droppedMeasurements += discarded;
  }

  const payload = {
    nodeId: cfg.nodeId,
    crMS: Date.now(),
    pubMS: Date.now(),
    bseq: bseq + 1,
    data: batch,
    drop: droppedMeasurements,
    skip: skippedFlushesForRateLimit,
  };

  let published = false;

  try {
    published = channel.publish(
      cfg.dataExchange,
      `node.${cfg.nodeId}`,
      Buffer.from(JSON.stringify(payload)),
      {
        contentType: "application/json",
        persistent: true,
        timestamp: Date.now(),
      },
    );
  } catch (error) {
    console.error("[publish] failed", error);
    reportError("publish-failed", "failed to publish measurement batch", true);
    pending = [...batch, ...pending];
    return;
  }

  if (!published) {
    pending = [...batch, ...pending];
    return;
  }

  sentThisWindow += 1;
  sentMessages += 1;
  bseq += 1;
}

async function configureSerial(): Promise<void> {
  const result = await $`stty -F ${cfg.serialDevice} ${String(cfg.serialBaud)} raw -echo -echoe -echok -echoctl -echoke`.nothrow();
  if (result.exitCode !== 0) {
    reportError("serial-config-failed", `failed to configure serial device ${cfg.serialDevice}`, true, {
      serialDevice: cfg.serialDevice,
      serialBaud: cfg.serialBaud,
    });
    throw new Error(`failed to configure serial device ${cfg.serialDevice}`);
  }
}

async function runSerialReaderOnce(): Promise<void> {
  await configureSerial();

  const proc = Bun.spawn(["cat", cfg.serialDevice], {
    stdout: "pipe",
    stderr: "pipe",
  });

  if (!proc.stdout) {
    throw new Error("serial reader did not expose stdout");
  }

  const reader = proc.stdout.getReader();
  const decoder = new TextDecoder();
  let carry = "";

  while (!isShuttingDown) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }

    carry += decoder.decode(value, { stream: true });
    const lines = carry.split(/\r?\n/);
    carry = lines.pop() ?? "";

    for (const rawLine of lines) {
      const measurement = parseMeasurement(rawLine.trim());
      if (measurement) {
        pushMeasurement(measurement);
      }
    }
  }

  const exitCode = await proc.exited;
  if (!isShuttingDown && exitCode !== 0) {
    const stderr = proc.stderr ? await new Response(proc.stderr).text() : "";
    reportError("serial-reader-exit", "serial reader exited unexpectedly", true, {
      exitCode,
      stderr: stderr.trim(),
    });
    throw new Error(`serial reader exited with code ${String(exitCode)} ${stderr.trim()}`);
  }
}

async function runSerialLoop(): Promise<void> {
  let attempt = 0;

  while (!isShuttingDown) {
    try {
      await runSerialReaderOnce();
      attempt = 0;
    } catch (error) {
      serialReconnects += 1;
      attempt += 1;
      const delayMs = nextBackoffMs(attempt);
      console.error(`[serial] disconnected, retrying in ${String(delayMs)}ms`, error);
      reportError("serial-disconnected", "serial reader disconnected, retrying", true, {
        attempt,
        delayMs,
      });
      await sleep(delayMs);
    }
  }
}

async function setupAmqpConnection(connection: Connection): Promise<{ dataChannel: Channel; errorPubChannel: Channel }> {
  const ch = await connection.createChannel();
  const errCh = await connection.createChannel();

  await ch.assertExchange(cfg.dataExchange, "topic", { durable: true });
  await ch.assertExchange(cfg.controlExchange, "topic", { durable: true });
  await errCh.assertExchange(cfg.errorExchange, "topic", { durable: true });

  const controlQueue = `uwb.control.${cfg.nodeId}`;
  await ch.assertQueue(controlQueue, { durable: true });
  await ch.bindQueue(controlQueue, cfg.controlExchange, `node.${cfg.nodeId}`);
  await ch.bindQueue(controlQueue, cfg.controlExchange, "node.all");

  await ch.consume(controlQueue, (msg: ConsumeMessage | null) => {
    if (!msg) {
      return;
    }

    try {
      const body = JSON.parse(msg.content.toString()) as ControlMessage;
      applyControl(body);
      ch.ack(msg);
    } catch (error) {
      console.error("[control] invalid payload", error);
      reportError("invalid-control-payload", "received invalid control payload", false);
      ch.nack(msg, false, false);
    }
  });

  return { dataChannel: ch, errorPubChannel: errCh };
}

async function runAmqpLoop(): Promise<void> {
  let attempt = 0;

  while (!isShuttingDown) {
    try {
      const activeConn = await amqp.connect(cfg.amqpUrl);
      const setup = await setupAmqpConnection(activeConn);
      const activeChannel = setup.dataChannel;
      const activeErrorChannel = setup.errorPubChannel;

      conn = activeConn;
      channel = activeChannel;
      errorChannel = activeErrorChannel;
      attempt = 0;

      await new Promise<void>((resolve) => {
        const cleanup = () => {
          if (conn === activeConn) {
            conn = null;
          }
          if (channel === activeChannel) {
            channel = null;
          }
          if (errorChannel === activeErrorChannel) {
            errorChannel = null;
          }
          resolve();
        };

        activeConn.once("close", cleanup);
        activeConn.once("error", (error: unknown) => {
          console.error("[amqp] connection error", error);
        });
      });
    } catch (error) {
      console.error("[amqp] connection/setup failed", error);
    }

    if (isShuttingDown) {
      break;
    }

    amqpReconnects += 1;
    attempt += 1;
    const delayMs = nextBackoffMs(attempt);
    console.log(`[amqp] reconnecting in ${String(delayMs)}ms`);
    reportError("amqp-disconnected", "amqp connection lost, retrying", true, {
      attempt,
      delayMs,
    });
    await sleep(delayMs);
  }
}

async function main(): Promise<void> {
  setFlushTimer();

  setInterval(() => {
    console.log("[stats]", {
      pending: pending.length,
      sentMessages,
      droppedMeasurements,
      skippedFlushesForRateLimit,
      serialReconnects,
      amqpReconnects,
      limits,
    });
  }, 5000);

  await Promise.all([runAmqpLoop(), runSerialLoop()]);
}

main().catch((error) => {
  console.error("publisher failed", error);
  process.exit(1);
});

process.on("SIGINT", async () => {
  isShuttingDown = true;

  try {
    if (flushTimer) {
      clearInterval(flushTimer);
    }
    await errorChannel?.close();
    await channel?.close();
    await conn?.close();
  } catch (error) {
    console.error(error);
  } finally {
    process.exit(0);
  }
});
