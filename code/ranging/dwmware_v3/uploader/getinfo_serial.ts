import { SerialPort } from "serialport";
import { ReadlineParser } from "@serialport/parser-readline";

const PORT_PATH = "/dev/ttyUSB0";
const BAUD_RATE = 115200;

async function main() {
    const port = new SerialPort({
        path: PORT_PATH,
        baudRate: BAUD_RATE,
    });

    await new Promise<void>((resolve, reject) => {
        port.once("open", resolve);
        port.once("error", reject);
    });

    console.log(`Port ${PORT_PATH} opened at ${BAUD_RATE} baud.`);

    // await new Promise(resolve => setTimeout(resolve, 1000));

    const parser = port.pipe(
        new ReadlineParser({
            delimiter: "\n",
            encoding: "ascii",
        })
    );

    console.log('Sending "AT+GETINFO"...');
    port.write("AT+GETINFO\n");

    const timeout = setTimeout(() => {
        console.log("Timeout reached.");
        port.close();
    }, 2000);

    parser.on("data", (line: string) => {
        line = line.trim();

        if (line.startsWith("INFO=")) {
            console.log("INFO line returned:", line);

            clearTimeout(timeout);
            port.close();
        }
    });

    port.on("error", (error) => {
        clearTimeout(timeout);
        console.error("Serial error:", error);
    });
}

main().catch(console.error);