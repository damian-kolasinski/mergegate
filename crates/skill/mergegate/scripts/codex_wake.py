"""Wakes a Codex thread for a mergegate item: the wake command a Codex agent
leaves with `mg submit --wake`.

usage: python3 codex_wake.py THREAD_ID

Queues a message telling the thread to read the item's events, which a Codex
app server that has the thread loaded runs as a new turn. The Codex app's
daemon loads only the threads a window shows, so where it runs this also
resumes the thread on it, until the turn starts.

CODEX_BIN names the codex binary when it isn't on PATH.
"""
import base64
import json
import os
import select
import shutil
import subprocess
import sys
import time

DAEMON = os.path.expanduser("~/.codex/app-server-control/app-server-control.sock")
# How long the daemon gets to start the queued turn once the thread loads.
STARTED_WITHIN = 60


def codex():
    found = os.environ.get("CODEX_BIN") or shutil.which("codex")
    for path in (found, os.path.expanduser("~/.local/bin/codex"),
                 "/Applications/Codex.app/Contents/Resources/codex"):
        if path and os.access(path, os.X_OK):
            return path
    sys.exit("codex_wake: no codex binary; set CODEX_BIN")


def message():
    item = os.environ.get("MG_ITEM", "the item")
    cursor = os.environ.get("MG_CURSOR")
    after = f" (newest cursor {cursor})" if cursor else ""
    return (f"mergegate: {item} has events you haven't read{after}. "
            f"Run `mg feedback {item}` and act on what it returns; the wake stays set, so end "
            "your turn when you're done instead of waiting.")


class Socket:
    """JSON-RPC over the WebSocket `codex app-server proxy` bridges to the
    daemon."""

    def __init__(self, binary):
        self.proc = subprocess.Popen([binary, "app-server", "proxy"], stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=sys.stderr, bufsize=0)
        self.buf = b""
        self.next_id = 0
        key = base64.b64encode(os.urandom(16)).decode()
        self.proc.stdin.write(
            ("GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
             f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        while b"\r\n\r\n" not in self.buf:
            if not self.read(10):
                raise RuntimeError("the daemon closed the connection before the handshake")
        head, _, self.buf = self.buf.partition(b"\r\n\r\n")
        if b" 101 " not in head.split(b"\r\n")[0]:
            raise RuntimeError(f"the daemon refused the WebSocket: {head[:200]!r}")

    def read(self, timeout):
        ready, _, _ = select.select([self.proc.stdout], [], [], timeout)
        if not ready:
            return True
        chunk = os.read(self.proc.stdout.fileno(), 65536)
        self.buf += chunk
        return bool(chunk)

    def send(self, method, params=None, request=True):
        msg = {"method": method}
        if params is not None:
            msg["params"] = params
        if request:
            self.next_id += 1
            msg["id"] = self.next_id
        data = json.dumps(msg).encode()
        head = bytearray([0x81])
        if len(data) < 126:
            head.append(0x80 | len(data))
        elif len(data) < 65536:
            head += bytes([0x80 | 126]) + len(data).to_bytes(2, "big")
        else:
            head += bytes([0x80 | 127]) + len(data).to_bytes(8, "big")
        mask = os.urandom(4)
        self.proc.stdin.write(bytes(head) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
        return msg.get("id")

    def messages(self):
        while len(self.buf) >= 2:
            size, offset = self.buf[1] & 0x7F, 2
            if size == 126:
                size, offset = int.from_bytes(self.buf[2:4], "big"), 4
            elif size == 127:
                size, offset = int.from_bytes(self.buf[2:10], "big"), 10
            if len(self.buf) < offset + size:
                return
            opcode, payload = self.buf[0] & 0x0F, self.buf[offset:offset + size]
            self.buf = self.buf[offset + size:]
            if opcode == 1:
                yield json.loads(payload)

    def until(self, seconds, done):
        end = time.time() + seconds
        while time.time() < end:
            if not self.read(min(1, max(0, end - time.time()))):
                return None
            for msg in self.messages():
                if "error" in msg:
                    raise RuntimeError(f"the daemon answered {msg['error']}")
                if done(msg):
                    return msg
        return None

    def close(self):
        self.proc.stdin.close()
        self.proc.terminate()


def resume(binary, thread):
    socket = Socket(binary)
    try:
        rid = socket.send("initialize", {"clientInfo": {"name": "mergegate", "title": "mergegate",
                                                        "version": "1"}})
        if not socket.until(10, lambda m: m.get("id") == rid):
            raise RuntimeError("the daemon didn't answer initialize")
        socket.send("initialized", request=False)
        rid = socket.send("thread/resume", {"threadId": thread, "excludeTurns": True})
        # The daemon tells every client about every thread it runs.
        def turn(m):
            return m.get("method") == "turn/started" and m.get("params", {}).get("threadId") == thread

        answer = socket.until(30, lambda m: turn(m) or m.get("id") == rid)
        if not answer:
            raise RuntimeError("the daemon didn't answer thread/resume")
        # A thread the daemon already had loaded starts the queued turn as
        # `codex queue` runs, before this connects to hear it: the resume
        # finds it running. One that wasn't loaded starts it after.
        status = answer.get("result", {}).get("thread", {}).get("status", {})
        started = (turn(answer) or status.get("type") == "active"
                   or socket.until(STARTED_WITHIN, turn))
        if not started:
            print(f"codex_wake: {thread} loaded, but no turn started within {STARTED_WITHIN}s; "
                  "the message stays queued", file=sys.stderr)
    finally:
        socket.close()


def main():
    if len(sys.argv) != 2 or not sys.argv[1]:
        sys.exit("usage: python3 codex_wake.py THREAD_ID")
    thread, binary = sys.argv[1], codex()
    subprocess.run([binary, "queue", "--thread", thread, "--message", message()],
                   stdout=subprocess.DEVNULL, check=True)
    if os.path.exists(DAEMON):
        try:
            resume(binary, thread)
        except (OSError, RuntimeError) as e:
            sys.exit(f"codex_wake: the message is queued, but loading {thread} failed: {e}")


main()
