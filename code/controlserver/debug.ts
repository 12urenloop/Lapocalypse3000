import net from "net";

const PORT = 7007;

// Track connected clients
const clients : Set<net.Socket> = new Set();

const server = net.createServer((socket) => {
    console.log("Client connected:", socket.remoteAddress, socket.remotePort);

    clients.add(socket);

    socket.on("data", (data) => {
        // data is a Node.js Buffer (works in Bun)

        if (data.length < 4) {
            console.log("Not enough bytes yet:", data);
            return;
        }else if(data.length > 16) {
            const json = JSON.parse(data.toString('utf-8'));
            const anchor10 = json.anchors.A1;
            console.log(anchor10);
            return;
        }

        // Read uint32 in big-endian
        const value = data.readUInt32LE(0);
        const valuehex = value.toString(16).toUpperCase().padStart(8, '0');
        const binaryvalue = value.toString(2).padStart(32, '0');

        console.log("Received uint32:", value, "hex=", valuehex, "binary=", binaryvalue);
    });

    socket.on("close", () => {
        console.log("Client disconnected");
        clients.delete(socket);
    });

    socket.on("error", (err) => {
        console.error("Socket error:", err);
        clients.delete(socket);
    });
});

// Start server
server.listen(PORT, () => {
    console.log(`TCP server listening on port ${PORT}`);
});


function formatvalue(val: string, index: number) : string {
    if(index === 0) return val; // first value is command
    if(val.endsWith('b')) {
        // binary
        const num = parseInt(val.slice(0, -1), 2);
        return num.toString();
    }else if(val.endsWith('h')) {
        // hex
        const num = parseInt(val.slice(0, -1), 16);
        return num.toString();
    }else{
        return val;
    }
}
// ----------------------------------------------------------------------
// SEND COMMAND TO ALL CONNECTED CLIENTS
// ----------------------------------------------------------------------
function sendCommandToAll(cmd : string) {
    const message = cmd.trim();   // ESP32 expects newline
    const formattedmsg = message.split(' ').map((v, i) => formatvalue(v, i)).join(' ');
    for (const c of clients) {
        c.write(formattedmsg + "\n");
    }
}


// ----------------------------------------------------------------------
// READ COMMANDS FROM STDIN (Bun’s stdin is a WebStream)
// ----------------------------------------------------------------------
const decoder = new TextDecoder();
const reader = Bun.stdin.stream().getReader();

console.log("Type commands to send to ESP32 clients:");

async function handleStdin() {
    while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        const text = decoder.decode(value).trim();
        if (text.length > 0) sendCommandToAll(text);
    }
}

handleStdin();
