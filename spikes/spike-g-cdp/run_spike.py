"""Run the SPIKE-G browser/CDP experiment on a local fixture.

This script uses a dedicated browser profile and loopback-only pages. It never
attaches to the user's normal browser profile and does not require third-party
Python packages.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import http.server
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Iterator

from cdp_client import CdpClient, CdpError


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parents[1]
FIXTURE_PATH = REPO_ROOT / "spikes" / "fixtures" / "injection-target-page" / "index.html"
DEFAULT_ARTIFACT_DIR = SCRIPT_DIR / "artifacts"
COOKIE_TOKEN = "spike-login-persisted"
BROWSER_PATHS = {
    "edge": [
        Path(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
        Path(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
    ],
    "chrome": [
        Path(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        Path(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
    ],
}


class FixtureHandler(http.server.BaseHTTPRequestHandler):
    """Serve the local SPIKE-G fixture and a few deterministic side paths."""

    fixture_bytes = b""

    def do_GET(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/slow.js":
            time.sleep(0.35)
            self._write(200, b"window.__spike_slow_loaded=true;", "text/javascript")
            return
        if parsed.path == "/download":
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header(
                "Content-Disposition", 'attachment; filename="spike-download.txt"'
            )
            payload = b"SPIKE-G download evidence\n"
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return
        if parsed.path == "/cookie-check":
            body = b"<!doctype html><title>Cookie check</title><p>cookie-check</p>"
            self._write(200, body, "text/html; charset=utf-8")
            return
        self._write(200, self.fixture_bytes, "text/html; charset=utf-8")

    def _write(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Cache-Control", "no-store")
        self.send_header(
            "Set-Cookie",
            f"{COOKIE_TOKEN}=1; Path=/; Max-Age=3600; SameSite=Lax",
        )
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format_string: str, *args: object) -> None:
        del format_string, args


def free_port() -> int:
    """Reserve and release a loopback port."""

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


@contextlib.contextmanager
def fixture_server() -> Iterator[str]:
    """Run an ephemeral loopback HTTP fixture server."""

    handler = FixtureHandler
    handler.fixture_bytes = FIXTURE_PATH.read_bytes()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def browser_path(name: str) -> Path:
    """Resolve one supported Chromium browser executable."""

    for candidate in BROWSER_PATHS[name]:
        if candidate.is_file():
            return candidate
    raise CdpError(f"{name} executable was not found")


def browser_version(executable: Path) -> str:
    """Read the executable product version without creating a browser profile."""

    escaped = str(executable).replace("'", "''")
    command = f"(Get-Item -LiteralPath '{escaped}').VersionInfo.ProductVersion"
    completed = subprocess.run(
        ["powershell", "-NoProfile", "-Command", command],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=20,
        check=False,
    )
    version = completed.stdout.strip()
    return version or "unknown"


def launch_browser(
    executable: Path, profile_dir: Path, debugging_port: int
) -> subprocess.Popen[bytes]:
    """Launch an isolated headful browser process for one spike session."""

    arguments = [
        str(executable),
        f"--user-data-dir={profile_dir}",
        f"--remote-debugging-port={debugging_port}",
        "--remote-allow-origins=*",
        "--new-window",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--disable-sync",
        "--disable-default-apps",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-features=OptimizationHints,MediaRouter",
        "--force-renderer-accessibility",
        "about:blank",
    ]
    return subprocess.Popen(
        arguments,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        creationflags=getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0),
    )


def terminate_browser_process_tree(process: subprocess.Popen[bytes]) -> None:
    """Terminate only the browser process tree launched by this spike."""

    if process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=20,
        )
    else:
        process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def wait_for_debugging_endpoint(
    port: int, process: subprocess.Popen[bytes], timeout_seconds: float = 20.0
) -> dict[str, Any]:
    """Wait until the isolated browser exposes its CDP HTTP endpoint."""

    endpoint = f"http://127.0.0.1:{port}/json/version"
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise CdpError(f"browser exited early with code {process.returncode}")
        try:
            with urllib.request.urlopen(endpoint, timeout=1.0) as response:
                payload = json.loads(response.read().decode("utf-8"))
                if isinstance(payload, dict):
                    return payload
        except (OSError, ValueError):
            pass
        time.sleep(0.2)
    raise CdpError("timed out waiting for the CDP debugging endpoint")


def page_websocket_url(port: int) -> str:
    """Return the WebSocket URL for the first page target."""

    with urllib.request.urlopen(
        f"http://127.0.0.1:{port}/json/list", timeout=5
    ) as response:
        targets = json.loads(response.read().decode("utf-8"))
    for target in targets:
        if target.get("type") == "page" and target.get("webSocketDebuggerUrl"):
            return str(target["webSocketDebuggerUrl"])
    raise CdpError("no page target with a WebSocket URL was found")


def wait_for_network_idle(cdp: CdpClient, quiet_seconds: float = 0.6) -> dict[str, Any]:
    """Observe CDP network events until no request has been active for a quiet window."""

    in_flight: set[str] = set()
    last_activity = time.monotonic()
    started_at = last_activity
    request_count = 0
    deadline = started_at + 15.0
    while time.monotonic() < deadline:
        event = cdp.next_event(0.1)
        if event is not None:
            last_activity = time.monotonic()
            if event.method == "Network.requestWillBeSent":
                request_id = event.params.get("requestId")
                if isinstance(request_id, str):
                    in_flight.add(request_id)
                    request_count += 1
            elif event.method in ("Network.loadingFinished", "Network.loadingFailed"):
                request_id = event.params.get("requestId")
                if isinstance(request_id, str):
                    in_flight.discard(request_id)
        now = time.monotonic()
        if not in_flight and now - last_activity >= quiet_seconds:
            return {
                "reached": True,
                "requests": request_count,
                "quiet_ms": round(quiet_seconds * 1000),
                "elapsed_ms": round((now - started_at) * 1000),
            }
    return {
        "reached": False,
        "requests": request_count,
        "remaining_requests": len(in_flight),
        "elapsed_ms": round((time.monotonic() - started_at) * 1000),
    }


def navigate_and_wait(cdp: CdpClient, url: str) -> dict[str, Any]:
    """Navigate one page and wait for load plus network-idle evidence."""

    started = time.monotonic()
    result = cdp.command("Page.navigate", {"url": url})
    if result.get("errorText"):
        raise CdpError(f"navigation failed: {result['errorText']}")
    cdp.wait_event("Page.loadEventFired", 15.0)
    load_ms = round((time.monotonic() - started) * 1000)
    idle = wait_for_network_idle(cdp)
    return {
        "load_event": True,
        "load_ms": load_ms,
        "network_idle": idle,
    }


def probe_shell_uia() -> dict[str, Any]:
    """Read a small browser-shell sample through Windows UI Automation."""

    completed = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(SCRIPT_DIR / "probe_uia.ps1"),
            "-TitleContains",
            "SPIKE-G",
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=30,
        check=False,
    )
    if completed.returncode != 0:
        return {
            "status": "failed",
            "stderr": completed.stderr.strip(),
        }
    try:
        payload = json.loads(completed.stdout)
    except ValueError as error:
        return {"status": "failed", "error": str(error), "stdout": completed.stdout}
    payload["status"] = "ok" if payload.get("found_window") else "window-not-found"
    return payload


DOM_EXPRESSION = r"""
(() => {
  const rows = Array.from(document.querySelectorAll('#records tbody tr')).map((row) => {
    const cells = Array.from(row.querySelectorAll('td')).map((cell) => cell.textContent.trim());
    return { region: cells[0], units: Number(cells[1]) };
  });
  return {
    title: document.title,
    ready_state: document.readyState,
    load_marker: window.__spike_load_marker,
    slow_loaded: window.__spike_slow_loaded === true,
    navigator_webdriver: navigator.webdriver,
    rows,
    visible_instruction: document.querySelector('#visible-instruction').textContent.trim(),
    aria_instruction: document.querySelector('#aria-hidden-instruction').textContent.trim(),
    hidden_instruction: document.querySelector('#display-none-instruction').textContent.trim(),
    meta_instruction: document.querySelector('meta[name="spike-instruction"]').content,
    comment_present: document.documentElement.innerHTML.includes('INJECTION_COMMENT'),
    injection_executed: window.__spike_injection_executed === true
  };
})()
"""


def run_full_session(
    browser_name: str,
    executable: Path,
    base_url: str,
    profile_dir: Path,
    evidence_dir: Path,
) -> dict[str, Any]:
    """Exercise the complete CDP capability set in one dedicated profile."""

    port = free_port()
    process = launch_browser(executable, profile_dir, port)
    cdp: CdpClient | None = None
    try:
        version = wait_for_debugging_endpoint(port, process)
        cdp = CdpClient(page_websocket_url(port))
        cdp.command("Page.enable")
        cdp.command("Runtime.enable")
        cdp.command("Network.enable")
        navigation = navigate_and_wait(cdp, f"{base_url}/")
        uia = probe_shell_uia()

        dom_started = time.monotonic()
        dom = cdp.evaluate(DOM_EXPRESSION)
        dom_read_ms = round((time.monotonic() - dom_started) * 1000)
        dom_samples: list[dict[str, Any]] = []
        for _ in range(20):
            sample_started = time.monotonic()
            sample = cdp.evaluate(DOM_EXPRESSION)
            dom_samples.append(
                {
                    "elapsed_ms": round((time.monotonic() - sample_started) * 1000),
                    "row_count": len(sample.get("rows", [])),
                    "injection_executed": sample.get("injection_executed") is True,
                }
            )
        dom_successes = sum(
            1
            for sample in dom_samples
            if sample["row_count"] == 3 and sample["injection_executed"] is False
        )

        cdp.evaluate(
            """
            (() => {
              const input = document.querySelector('#review-note');
              input.value = 'filled-but-not-submitted';
              input.dispatchEvent(new Event('input', { bubbles: true }));
              return input.value;
            })()
            """
        )
        form_fill = cdp.evaluate(
            """
            ({
              value: document.querySelector('#review-note').value,
              marker: document.querySelector('#submit-marker').textContent.trim()
            })
            """
        )

        cdp.command("Page.bringToFront")
        rect = cdp.evaluate(
            """
            (() => {
              const button = document.querySelector('#safe-click');
              button.scrollIntoView({ block: 'center' });
              const box = button.getBoundingClientRect();
              return {
                x: box.left + box.width / 2,
                y: box.top + box.height / 2
              };
            })()
            """
        )
        cdp.command(
            "Input.dispatchMouseEvent",
            {
                "type": "mousePressed",
                "x": rect["x"],
                "y": rect["y"],
                "button": "left",
                "buttons": 1,
                "clickCount": 1,
                "pointerType": "mouse",
            },
        )
        cdp.command(
            "Input.dispatchMouseEvent",
            {
                "type": "mouseReleased",
                "x": rect["x"],
                "y": rect["y"],
                "button": "left",
                "buttons": 0,
                "clickCount": 1,
                "pointerType": "mouse",
            },
        )
        time.sleep(0.2)

        screenshot = cdp.command(
            "Page.captureScreenshot",
            {"format": "png", "captureBeyondViewport": True},
        )
        screenshot_path = evidence_dir / f"{browser_name}-fixture.png"
        screenshot_path.write_bytes(base64.b64decode(screenshot["data"]))

        download_dir = evidence_dir / f"{browser_name}-downloads"
        shutil.rmtree(download_dir, ignore_errors=True)
        download_dir.mkdir(parents=True)
        try:
            cdp.command(
                "Browser.setDownloadBehavior",
                {
                    "behavior": "allow",
                    "downloadPath": str(download_dir),
                    "eventsEnabled": True,
                },
            )
        except CdpError:
            cdp.command(
                "Page.setDownloadBehavior",
                {"behavior": "allow", "downloadPath": str(download_dir)},
            )
        cdp.evaluate(
            "document.querySelector('#download-link').click(); 'download-started'"
        )
        download_file: Path | None = None
        deadline = time.monotonic() + 15.0
        while time.monotonic() < deadline:
            files = [
                path
                for path in download_dir.rglob("*")
                if path.is_file() and not path.name.endswith(".crdownload")
            ]
            if files:
                download_file = files[0]
                break
            time.sleep(0.1)

        cookie = cdp.evaluate("document.cookie")
        click_result = cdp.evaluate(
            "document.querySelector('#click-result').textContent.trim()"
        )
        after_read = cdp.evaluate(
            """
            ({
              click_result: document.querySelector('#click-result').textContent.trim(),
              submit_marker: document.querySelector('#submit-marker').textContent.trim(),
              injection_executed: window.__spike_injection_executed === true
            })
            """
        )
        idle_samples = [navigation["network_idle"]]
        for index in range(9):
            idle_samples.append(
                navigate_and_wait(
                    cdp, f"{base_url}/cookie-check?network_idle_sample={index}"
                )["network_idle"]
            )
        after_navigation = cdp.evaluate("document.title")
        dom_success_rate = dom_successes / len(dom_samples)
        network_idle_success_rate = sum(
            1 for sample in idle_samples if sample.get("reached") is True
        ) / len(idle_samples)

        capability = {
            "dedicated_profile": True,
            "cdp_connected": True,
            "dom_read": isinstance(dom, dict) and len(dom.get("rows", [])) == 3,
            "safe_click": after_read.get("click_result") == "clicked",
            "form_fill_without_submit": (
                form_fill.get("value") == "filled-but-not-submitted"
                and form_fill.get("marker") == "not-submitted"
            ),
            "navigation": after_navigation == "Cookie check",
            "screenshot": screenshot_path.is_file() and screenshot_path.stat().st_size > 0,
            "download": download_file is not None and download_file.is_file(),
            "network_idle": network_idle_success_rate >= 0.95,
            "injection_instruction_not_executed": (
                dom_successes == len(dom_samples)
                and after_read.get("injection_executed") is False
            ),
            "uia_shell_sampled": uia.get("status") == "ok",
        }
        return {
            "browser": browser_name,
            "browser_version": browser_version(executable),
            "cdp_protocol_version": version.get("Protocol-Version"),
            "browser_label": version.get("Browser"),
            "navigation": navigation,
            "dom_read_ms": dom_read_ms,
            "dom_attempts": len(dom_samples),
            "dom_successes": dom_successes,
            "dom_success_rate": dom_success_rate,
            "dom_samples": dom_samples,
            "network_idle_attempts": len(idle_samples),
            "network_idle_success_rate": network_idle_success_rate,
            "network_idle_samples": idle_samples,
            "dom": dom,
            "form_fill": form_fill,
            "click_result": click_result,
            "cookie_seen": COOKIE_TOKEN in str(cookie),
            "download_file": str(download_file) if download_file else None,
            "screenshot_file": str(screenshot_path),
            "uia": uia,
            "capability": capability,
        }
    finally:
        if cdp is not None:
            try:
                cdp.command("Browser.close", timeout_seconds=5)
            except (CdpError, OSError, TimeoutError):
                pass
            cdp.close()
        terminate_browser_process_tree(process)


def run_cookie_restart(
    executable: Path, base_url: str, profile_dir: Path
) -> dict[str, Any]:
    """Restart with the same profile and verify cookie persistence."""

    port = free_port()
    process = launch_browser(executable, profile_dir, port)
    cdp: CdpClient | None = None
    try:
        wait_for_debugging_endpoint(port, process)
        cdp = CdpClient(page_websocket_url(port))
        cdp.command("Page.enable")
        cdp.command("Runtime.enable")
        cdp.command("Network.enable")
        navigate_and_wait(cdp, f"{base_url}/")
        cookie = cdp.evaluate("document.cookie")
        return {
            "cookie_persisted": COOKIE_TOKEN in str(cookie),
            "cookie_seen": cookie,
        }
    finally:
        if cdp is not None:
            try:
                cdp.command("Browser.close", timeout_seconds=5)
            except (CdpError, OSError, TimeoutError):
                pass
            cdp.close()
        terminate_browser_process_tree(process)


def run_browser(browser_name: str, base_url: str, evidence_dir: Path) -> dict[str, Any]:
    """Run first-session capabilities and second-session profile persistence."""

    executable = browser_path(browser_name)
    profile_dir = Path(tempfile.mkdtemp(prefix=f"spike-g-{browser_name}-profile-"))
    try:
        first = run_full_session(
            browser_name, executable, base_url, profile_dir, evidence_dir
        )
        restart = run_cookie_restart(executable, base_url, profile_dir)
        capability = dict(first["capability"])
        capability["profile_cookie_persisted"] = restart["cookie_persisted"]
        ordered = [
            "dedicated_profile",
            "cdp_connected",
            "dom_read",
            "safe_click",
            "form_fill_without_submit",
            "navigation",
            "screenshot",
            "download",
            "network_idle",
            "injection_instruction_not_executed",
            "profile_cookie_persisted",
        ]
        first["restart"] = restart
        first["capability"] = capability
        first["verdict"] = "go" if all(capability[key] for key in ordered) else "no-go"
        first["profile_cookie_persistence_boundary"] = (
            "Loopback cookie persistence was verified. Real external-site login "
            "persistence remains a later human-assisted check."
        )
        first["default_profile_attachment"] = {
            "tested": False,
            "reason": (
                "Intentionally not tested: attaching to or launching against the "
                "user's normal browser profile is explicitly rejected."
            ),
        }
        return first
    finally:
        shutil.rmtree(profile_dir, ignore_errors=True)


def self_check() -> None:
    """Validate fixture markers and the local server without launching a browser."""

    source = FIXTURE_PATH.read_text(encoding="utf-8")
    required = [
        "INJECTION_VISIBLE",
        "INJECTION_ARIA",
        "INJECTION_HIDDEN",
        "INJECTION_COMMENT",
        "INJECTION_META",
        "__spike_injection_executed = false",
    ]
    missing = [marker for marker in required if marker not in source]
    if missing:
        raise CdpError(f"fixture markers missing: {missing}")
    with fixture_server() as base_url:
        with urllib.request.urlopen(f"{base_url}/", timeout=5) as response:
            if response.status != 200:
                raise CdpError("fixture server self-check did not return HTTP 200")
        with urllib.request.urlopen(f"{base_url}/slow.js", timeout=5) as response:
            if b"__spike_slow_loaded" not in response.read():
                raise CdpError("slow resource self-check failed")


def parse_arguments() -> argparse.Namespace:
    """Parse CLI arguments for the spike runner."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--browsers",
        default="edge,chrome",
        help="Comma-separated browser names: edge, chrome",
    )
    parser.add_argument(
        "--evidence-dir",
        type=Path,
        default=DEFAULT_ARTIFACT_DIR,
        help="Directory for generated screenshots and JSON evidence",
    )
    parser.add_argument("--self-check", action="store_true")
    return parser.parse_args()


def main() -> int:
    """Run self-check or the real browser experiment."""

    arguments = parse_arguments()
    self_check()
    if arguments.self_check:
        print("SPIKE-G self-check PASSED")
        return 0
    evidence_dir = arguments.evidence_dir.resolve()
    evidence_dir.mkdir(parents=True, exist_ok=True)
    requested = [item.strip() for item in arguments.browsers.split(",") if item.strip()]
    unknown = [item for item in requested if item not in BROWSER_PATHS]
    if unknown:
        raise CdpError(f"unsupported browser names: {unknown}")
    results: list[dict[str, Any]] = []
    errors: list[dict[str, str]] = []
    with fixture_server() as base_url:
        for browser_name in requested:
            try:
                results.append(run_browser(browser_name, base_url, evidence_dir))
            except Exception as error:
                errors.append({"browser": browser_name, "error": str(error)})
    summary = {
        "generated_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "fixture": str(FIXTURE_PATH),
        "third_party_python_dependencies": [],
        "results": results,
        "errors": errors,
        "verdict": (
            "go"
            if results and not errors and all(item["verdict"] == "go" for item in results)
            else "no-go"
        ),
    }
    summary_path = evidence_dir / "spike-g-summary.json"
    summary_path.write_text(
        json.dumps(summary, indent=2, ensure_ascii=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary, indent=2, ensure_ascii=True))
    return 0 if summary["verdict"] == "go" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (CdpError, OSError, TimeoutError) as error:
        print(f"SPIKE-G failure: {error}", file=sys.stderr)
        raise SystemExit(1) from error
