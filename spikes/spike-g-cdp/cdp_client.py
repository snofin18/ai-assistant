"""Minimal standard-library CDP client used only by SPIKE-G.

The spike deliberately avoids a third-party browser automation dependency. It is
not product code and does not attempt to be a general WebSocket implementation.
"""

from __future__ import annotations

import base64
import json
import os
import socket
import struct
import time
from dataclasses import dataclass
from typing import Any
from urllib.parse import urlparse


class CdpError(RuntimeError):
    """Raised when a CDP command or WebSocket exchange fails."""


@dataclass
class CdpEvent:
    """One asynchronous CDP event."""

    method: str
    params: dict[str, Any]


class WebSocketClient:
    """Small RFC 6455 client sufficient for localhost CDP text frames."""

    def __init__(self, url: str, timeout_seconds: float = 10.0) -> None:
        parsed = urlparse(url)
        if parsed.scheme != "ws" or parsed.hostname is None or parsed.port is None:
            raise CdpError(f"unsupported WebSocket URL: {url}")
        self._socket = socket.create_connection(
            (parsed.hostname, parsed.port), timeout=timeout_seconds
        )
        self._socket.settimeout(timeout_seconds)
        path = parsed.path or "/"
        if parsed.query:
            path = f"{path}?{parsed.query}"
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        request = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {parsed.hostname}:{parsed.port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n"
            "\r\n"
        )
        self._socket.sendall(request.encode("ascii"))
        response = self._read_until(b"\r\n\r\n")
        status_line = response.split(b"\r\n", 1)[0]
        if b" 101 " not in status_line:
            raise CdpError(f"WebSocket handshake failed: {status_line!r}")

    def _read_exact(self, count: int) -> bytes:
        chunks: list[bytes] = []
        remaining = count
        while remaining > 0:
            chunk = self._socket.recv(remaining)
            if not chunk:
                raise CdpError("WebSocket closed while reading")
            chunks.append(chunk)
            remaining -= len(chunk)
        return b"".join(chunks)

    def _read_until(self, marker: bytes) -> bytes:
        data = bytearray()
        while marker not in data:
            chunk = self._socket.recv(4096)
            if not chunk:
                raise CdpError("WebSocket closed during handshake")
            data.extend(chunk)
            if len(data) > 64 * 1024:
                raise CdpError("WebSocket handshake headers exceeded 64 KiB")
        return bytes(data)

    def _send_frame(self, opcode: int, payload: bytes) -> None:
        mask = os.urandom(4)
        header = bytearray([0x80 | opcode])
        length = len(payload)
        if length < 126:
            header.append(0x80 | length)
        elif length <= 0xFFFF:
            header.append(0x80 | 126)
            header.extend(struct.pack("!H", length))
        else:
            header.append(0x80 | 127)
            header.extend(struct.pack("!Q", length))
        header.extend(mask)
        masked = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
        self._socket.sendall(bytes(header) + masked)

    def send_text(self, text: str) -> None:
        """Send one unfragmented text frame."""

        self._send_frame(0x1, text.encode("utf-8"))

    def receive_text(self, timeout_seconds: float) -> str | None:
        """Receive one text message, or return None on timeout."""

        self._socket.settimeout(timeout_seconds)
        try:
            first, second = self._read_exact(2)
        except TimeoutError:
            return None
        opcode = first & 0x0F
        masked = bool(second & 0x80)
        length = second & 0x7F
        if length == 126:
            length = struct.unpack("!H", self._read_exact(2))[0]
        elif length == 127:
            length = struct.unpack("!Q", self._read_exact(8))[0]
        mask = self._read_exact(4) if masked else None
        payload = self._read_exact(length)
        if mask is not None:
            payload = bytes(
                value ^ mask[index % 4] for index, value in enumerate(payload)
            )
        if opcode == 0x8:
            raise CdpError("CDP WebSocket closed by peer")
        if opcode == 0x9:
            self._send_frame(0xA, payload)
            return self.receive_text(timeout_seconds)
        if opcode == 0xA:
            return ""
        if opcode != 0x1:
            raise CdpError(f"unsupported WebSocket opcode: {opcode}")
        return payload.decode("utf-8")

    def close(self) -> None:
        """Close the socket without attempting a graceful browser command."""

        try:
            self._send_frame(0x8, b"")
        except OSError:
            pass
        self._socket.close()


class CdpClient:
    """Synchronous CDP request/event client for one page target."""

    def __init__(self, websocket_url: str) -> None:
        self._websocket = WebSocketClient(websocket_url)
        self._next_id = 1
        self._events: list[CdpEvent] = []
        self._responses: dict[int, dict[str, Any]] = {}

    def _read_message(self, timeout_seconds: float) -> dict[str, Any] | None:
        raw = self._websocket.receive_text(timeout_seconds)
        if raw is None or raw == "":
            return None
        message = json.loads(raw)
        if not isinstance(message, dict):
            raise CdpError("CDP message was not a JSON object")
        message_id = message.get("id")
        if isinstance(message_id, int):
            self._responses[message_id] = message
        else:
            method = message.get("method")
            params = message.get("params", {})
            if isinstance(method, str) and isinstance(params, dict):
                self._events.append(CdpEvent(method, params))
        return message

    def command(
        self,
        method: str,
        params: dict[str, Any] | None = None,
        timeout_seconds: float = 15.0,
    ) -> dict[str, Any]:
        """Send one command and wait for its matching response."""

        command_id = self._next_id
        self._next_id += 1
        self._websocket.send_text(
            json.dumps({"id": command_id, "method": method, "params": params or {}})
        )
        deadline = time.monotonic() + timeout_seconds
        while command_id not in self._responses:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise CdpError(f"timeout waiting for CDP response: {method}")
            self._read_message(min(remaining, 0.25))
        response = self._responses.pop(command_id)
        error = response.get("error")
        if error is not None:
            raise CdpError(f"CDP command failed: {method}: {error}")
        result = response.get("result", {})
        if not isinstance(result, dict):
            raise CdpError(f"CDP result was not an object: {method}")
        return result

    def wait_event(self, method: str, timeout_seconds: float) -> dict[str, Any]:
        """Wait for the next matching event and return its parameters."""

        for index, event in enumerate(self._events):
            if event.method == method:
                return self._events.pop(index).params
        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            remaining = deadline - time.monotonic()
            self._read_message(min(remaining, 0.25))
            for index, event in enumerate(self._events):
                if event.method == method:
                    return self._events.pop(index).params
        raise CdpError(f"timeout waiting for CDP event: {method}")

    def next_event(self, timeout_seconds: float) -> CdpEvent | None:
        """Return one queued or newly read event, if available."""

        if self._events:
            return self._events.pop(0)
        self._read_message(timeout_seconds)
        if not self._events:
            return None
        return self._events.pop(0)

    def evaluate(self, expression: str) -> Any:
        """Evaluate one JavaScript expression and return a by-value result."""

        result = self.command(
            "Runtime.evaluate",
            {
                "expression": expression,
                "returnByValue": True,
                "awaitPromise": True,
            },
        )
        if result.get("exceptionDetails") is not None:
            raise CdpError(f"JavaScript evaluation failed: {result}")
        remote = result.get("result", {})
        if not isinstance(remote, dict):
            raise CdpError("Runtime.evaluate result was not an object")
        return remote.get("value")

    def close(self) -> None:
        """Close the CDP socket."""

        self._websocket.close()
