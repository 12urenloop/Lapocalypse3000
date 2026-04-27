# websocket-publisher

Bun service that consumes AMQP messages from `uwb.positions` and republishes them live over WebSocket.

The service binds to an existing RabbitMQ exchange named `uwb.positions` by default, so it does not redeclare that exchange.

## Input payload

Expected JSON message shape:

```json
{ "tag_id": "abc123", "x": 12.3, "y": 45.6 }
```

`x` and `y` are optional, but if present they must be numeric.

## Run

```bash
cp .env.example .env
./deploy.sh
```

WebSocket endpoint:

- `ws://localhost:3000/ws`

All connected clients receive the same live stream.
