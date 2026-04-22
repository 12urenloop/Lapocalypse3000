# UWB Aggregation Stack

This workspace contains three parts:

- `publisher-bun`: Raspberry Pi publisher that ingests UWB serial lines, batches data, and publishes to RabbitMQ.
- `rabbitmq-server`: RabbitMQ deployment and central control message sender.
- `consumer-simple`: Minimal consumer that persists incoming messages to a JSONL text file.

## 1) RabbitMQ server (central)

```bash
cd rabbitmq-server
cp .env.example .env
./deploy.sh
```

- AMQP endpoint: `amqp://localhost:5672`
- Management UI: `http://localhost:15672`

## 2) Consumer (central)

```bash
cd consumer-simple
cp .env.example .env
./deploy.sh
```

Output file defaults to:

- `consumer-simple/data/uwb-measurements.jsonl`

## 3) Publisher (each Raspberry Pi)

```bash
cd publisher-bun
cp .env.example .env
./deploy.sh
```

Expected serial line format:

```text
= <tag id> <distance in centimeter> mesh <synced time in millis> <unsynced millis>
```

Example:

```text
= T001 187 mesh 1717088850123 235661
```

## Runtime node control from central server

Use the control script from `rabbitmq-server` to change limits for one node or all nodes.

Single node:

```bash
cd rabbitmq-server
bun run send-control.ts --node pi-01 --mps 5 --max-per-message 25 --interval-ms 200
```

All nodes:

```bash
cd rabbitmq-server
bun run send-control.ts --node all --mps 3 --max-per-message 20 --interval-ms 300
```

Control fields:

- `--mps`: max messages per second per node.
- `--max-per-message`: max measurements in one published message (extra measurements are discarded).
- `--interval-ms`: publisher flush interval in milliseconds.

## Notes

- Publisher routing key: `node.<NODE_ID>`
- Consumer default binding: `node.*`
- Control routing key: `node.<NODE_ID>` or `node.all`
- Error exchange: `uwb.errors` (configurable with `ERROR_EXCHANGE`)

## Failure recovery behavior

The stack is designed to recover automatically from transient failures:

- Publisher:
	- Reconnects to RabbitMQ with exponential backoff and jitter when broker/network is down.
	- Retries serial ingestion loop when USB serial device is unplugged/replugged.
	- Keeps a bounded in-memory buffer (`MAX_PENDING_MEASUREMENTS`) during outages; oldest measurements are dropped when full.
	- Publishes structured error events to `ERROR_EXCHANGE` on a dedicated AMQP channel.
- Consumer:
	- Reconnects to RabbitMQ with exponential backoff and jitter.
	- Requeues messages when file persistence fails temporarily.
	- Drops malformed JSON payloads to avoid poison-message loops.
	- Publishes persistence/parse failures to `ERROR_EXCHANGE` on a dedicated AMQP channel.
- Control sender:
	- Retries connecting/publishing control messages with backoff.
	- Uses confirm channel (`waitForConfirms`) so successful command means broker accepted it.
	- Publishes control-publish failures to `ERROR_EXCHANGE` on a dedicated AMQP channel when available.

### Error events

Error events are published as JSON with fields like:

- `component`
- `type`
- `message`
- `recoverable`
- `details`
- `createdAtMs`

Example routing keys:

- `publisher.<NODE_ID>.<error-type>`
- `consumer.persist.<error-type>`
- `control.send.failed`

Create a queue for all error events (RabbitMQ UI or CLI) and bind it to `uwb.errors` with binding key `#`.

### Resilience tuning

Publisher settings in `publisher-bun/.env`:

- `MAX_PENDING_MEASUREMENTS` default `5000`
- `RECONNECT_MIN_MS` default `500`
- `RECONNECT_MAX_MS` default `10000`
- `ERROR_EXCHANGE` default `uwb.errors`

Consumer settings in `consumer-simple/.env`:

- `RECONNECT_MIN_MS` default `500`
- `RECONNECT_MAX_MS` default `10000`
- `ERROR_EXCHANGE` default `uwb.errors`

Control sender settings in `rabbitmq-server/.env`:

- `SEND_CONTROL_RETRY_COUNT` default `8`
- `SEND_CONTROL_RETRY_MIN_MS` default `300`
- `SEND_CONTROL_RETRY_MAX_MS` default `5000`
- `ERROR_EXCHANGE` default `uwb.errors`
