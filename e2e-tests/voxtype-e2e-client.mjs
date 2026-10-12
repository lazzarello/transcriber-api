import { spawn } from "node:child_process";
import fs from "node:fs";

const ws = new WebSocket("ws://127.0.0.1:8765/v1/stream");
const inputFile = process.argv[2];
let recorder;
let input;
let stopped = false;
let complete = false;

function stop() {
  if (stopped) return;
  stopped = true;

  if (recorder) recorder.kill("SIGINT");
  else if (ws.readyState === WebSocket.OPEN)
      ws.send(JSON.stringify({ type: "stop"}));
}

function sendAudio(chunk) {
  if (!stopped && ws.readyState === WebSocket.OPEN)
    // these chunks are definitely not aligned with what the server expects
    ws.send(chunk);
}

function startAudio() {
  if (inputFile) {
    // What is highWaterMark? How is this readstream sending chunks to the server?
    input = fs.createReadStream(inputFile, { highWaterMark: 3200 });
    input.on("data", sendAudio);
    input.on("end", stop);
  } else {
    recorder = spawn("pw-record", [
      "--raw",
      "--rate", "16000",
      "--channels", "1",
      "--format", "s16",
      "-"
    ]);

    recorder.stdout.on("data", sendAudio);
    recorder.on("close", stop);

    console.log("Speak now; recording for 8 seconds...");
    setTimeout(stop, 8000);
  }
}

ws.addEventListener("open", () => {
  ws.send(JSON.stringify({
    type: "start",
    protocol: 1,
    audio: {
      encoding: "pcm_s16le",
      sample_rate: 16000,
      channels: 1
    }
  }));
});

ws.addEventListener("message", event => {
  const message = JSON.parse(event.data);
  console.log(JSON.stringify(message));

  if (message.type === "ready") startAudio();

  if (message.type === "error") {
    console.error(`API error: ${message.code}: ${message.message}`);
    ws.close();
  }

  if (message.type === "complete") {
    complete = true;
    console.log(`FINAL: ${message.text}`);
    ws.close();
  }
});

ws.addEventListener("close", () => {
  if (!complete) process.exitCode = 1;
});
