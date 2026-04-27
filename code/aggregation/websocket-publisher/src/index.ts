import amqp, { type Channel, type Connection, type ConsumeMessage } from "amqplib";

type PositionMessage = {
  tag_id: string;
  x?: number;
  y?: number;
};

type PositionEnvelope = PositionMessage & {
  receivedAtMs: number;
};

type AppConfig = {
  amqpUrl: string;
  positionsExchange: string;
  positionsQueue: string;
  positionsBindingKey: string;
  wsHost: string;
  wsPort: number;
  wsPath: string;
  reconnectMinMs: number;
  reconnectMaxMs: number;
};

const cfg: AppConfig = {
  amqpUrl: process.env.AMQP_URL ?? "amqp://uwb:uwb@localhost:5672",
  positionsExchange: process.env.POSITIONS_EXCHANGE ?? "uwb.positions",
  positionsQueue: process.env.POSITIONS_QUEUE ?? "websocket-publisher.positions",
  positionsBindingKey: process.env.POSITIONS_BINDING_KEY ?? "#",
  wsHost: process.env.WS_HOST ?? "0.0.0.0",
  wsPort: Number(process.env.WS_PORT ?? "3000"),
  wsPath: process.env.WS_PATH ?? "/ws",
  reconnectMinMs: Number(process.env.RECONNECT_MIN_MS ?? "500"),
  reconnectMaxMs: Number(process.env.RECONNECT_MAX_MS ?? "10000"),
};

const websocketChannel = "positions";

let amqpConn: Connection | null = null;
let amqpChannel: Channel | null = null;
let shutdownRequested = false;
let connectedClients = 0;
let reconnectAttempt = 0;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function nextBackoffMs(attempt: number): number {
  const exp = Math.min(cfg.reconnectMaxMs, cfg.reconnectMinMs * 2 ** Math.max(0, attempt - 1));
  const jitter = Math.floor(Math.random() * Math.max(1, Math.floor(exp * 0.2)));
  return exp + jitter;
}

function parsePositionMessage(raw: string): PositionMessage {
  const parsed = JSON.parse(raw) as unknown;
  console.log(parsed);
  if (typeof parsed !== "object" || parsed === null) {
    throw new Error("message must be a JSON object");
  }

  const record = parsed as Record<string, unknown>;
  console.log(record);
  if (typeof record.tag_id !== "string" || record.tag_id.length === 0) {
    throw new Error("tag_id must be a non-empty string");
  }

  if (record.x !== undefined && typeof record.x !== "number") {
    throw new Error("x must be a number when present");
  }

  if (record.y !== undefined && typeof record.y !== "number") {
    throw new Error("y must be a number when present");
  }

  return {
    tag_id: record.tag_id,
    ...(typeof record.x === "number" ? { x: record.x } : {}),
    ...(typeof record.y === "number" ? { y: record.y } : {}),
  };
}

function publishToWebSockets(message: PositionMessage): void {
  const payload: PositionEnvelope = {
    ...message,
    receivedAtMs: Date.now(),
  };

  globalThis.websocketServer?.publish(websocketChannel, JSON.stringify(payload));
}

async function setupAmqp(connection: Connection): Promise<Channel> {
  const channel = await connection.createChannel();
  await channel.assertQueue(cfg.positionsQueue, { durable: true });
  await channel.bindQueue(cfg.positionsQueue, cfg.positionsExchange, cfg.positionsBindingKey);
  await channel.prefetch(512);

  await channel.consume(cfg.positionsQueue, (msg: ConsumeMessage | null) => {
    if (!msg) {
      return;
    }

    try {
      const parsed = parsePositionMessage(msg.content.toString());
      publishToWebSockets(parsed);
      channel.ack(msg);
    } catch (error) {
      console.error("[positions] dropping invalid message", error);
      channel.nack(msg, false, false);
    }
  });

  return channel;
}

async function runAmqpLoop(): Promise<void> {
  while (!shutdownRequested) {
    try {
      const connection = await amqp.connect(cfg.amqpUrl);
      const channel = await setupAmqp(connection);

      amqpConn = connection;
      amqpChannel = channel;
      reconnectAttempt = 0;

      console.log("[amqp] connected", {
        exchange: cfg.positionsExchange,
        queue: cfg.positionsQueue,
      });

      await new Promise<void>((resolve) => {
        const done = () => {
          if (amqpConn === connection) {
            amqpConn = null;
          }
          if (amqpChannel === channel) {
            amqpChannel = null;
          }
          resolve();
        };

        connection.once("close", done);
        connection.once("error", (error: unknown) => {
          console.error("[amqp] connection error", error);
        });
      });
    } catch (error) {
      console.error("[amqp] connection loop error", error);
    }

    if (shutdownRequested) {
      break;
    }

    reconnectAttempt += 1;
    const delayMs = nextBackoffMs(reconnectAttempt);
    console.log(`[amqp] reconnecting in ${String(delayMs)}ms`);
    await sleep(delayMs);
  }
}

const websocketServer = Bun.serve({
  hostname: cfg.wsHost,
  port: cfg.wsPort,
  fetch(req, server) {
    const url = new URL(req.url);

    if (url.pathname !== cfg.wsPath) {
      if (url.pathname === "/healthz") {
        return Response.json({
          ok: true,
          connectedClients,
          wsPath: cfg.wsPath,
          amqpConnected: amqpConn !== null,
        });
      }

      return new Response("Not found", { status: 404 });
    }

    const upgraded = server.upgrade(req, {
      data: {
        connectedAtMs: Date.now(),
      },
    });

    return upgraded ? undefined : new Response("Upgrade failed", { status: 400 });
  },
  websocket: {
    open(ws) {
      ws.subscribe(websocketChannel);
      connectedClients += 1;
      ws.send(
        JSON.stringify({
          type: "ready",
          channel: websocketChannel,
          connectedAtMs: Date.now(),
          connectedClients,
        }),
      );
    },
    close(_ws) {
      connectedClients = Math.max(0, connectedClients - 1);
    },
  },
});

(globalThis as typeof globalThis & { websocketServer?: typeof websocketServer }).websocketServer =
  websocketServer;

console.log("websocket publisher started", {
  wsUrl: `ws://${cfg.wsHost}:${cfg.wsPort}${cfg.wsPath}`,
  healthzUrl: `http://${cfg.wsHost}:${cfg.wsPort}/healthz`,
});

const shutdown = async (): Promise<void> => {
  shutdownRequested = true;

  try {
    await amqpChannel?.close();
  } catch {
    // ignore shutdown errors
  }

  try {
    await amqpConn?.close();
  } catch {
    // ignore shutdown errors
  }

  try {
    websocketServer.stop();
  } catch {
    // ignore shutdown errors
  }

  process.exit(0);
};

process.on("SIGINT", () => {
  void shutdown();
});

process.on("SIGTERM", () => {
  void shutdown();
});

runAmqpLoop().catch((error) => {
  console.error("websocket publisher failed", error);
  process.exit(1);
});
