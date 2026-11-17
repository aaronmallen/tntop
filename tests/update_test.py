import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest


class UpdateTest(unittest.TestCase):
    def test_quit_cancels_download_and_preserves_binary(self):
        for keys in [b"\x03", b"mq"]:
            with self.subTest(keys=keys), tempfile.TemporaryDirectory() as root:
                root = Path(root)
                binary = root / "tntop"
                shutil.copy2(os.environ["TEST_BINARY"], binary)
                original = hashlib.sha256(binary.read_bytes()).digest()
                commands = root / "commands"
                commands.mkdir()
                marker = root / "download"
                curl = commands / "curl"
                curl.write_text(
                    f"#!{sys.executable}\n"
                    "import json, os, pathlib, sys, time\n"
                    "destination = pathlib.Path(sys.argv[sys.argv.index('--output') + 1])\n"
                    "destination.write_text('partial download')\n"
                    "marker = pathlib.Path(os.environ['MARKER'])\n"
                    "marker.with_suffix('.tmp').write_text(json.dumps([os.getpid(), str(destination)]))\n"
                    "marker.with_suffix('.tmp').replace(marker)\n"
                    "time.sleep(30)\n"
                )
                curl.chmod(0o755)
                config = root / "config.toml"
                config.write_text("save_on_exit = false\n")
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
                process = subprocess.Popen(
                    [str(binary), "--config", str(config)],
                    stdin=slave, stdout=slave, stderr=slave,
                    env={
                        **{k: v for k, v in os.environ.items() if not k.startswith("TNTOP_")},
                        "PATH": f"{commands}:{os.environ['PATH']}",
                        "MARKER": str(marker), "TERM": "xterm-256color",
                    },
                )
                os.close(slave)
                try:
                    output = b""
                    deadline = time.monotonic() + 5
                    while b"servers" not in output.lower():
                        self.assertIsNone(process.poll(), output.decode(errors="replace"))
                        self.assertLess(time.monotonic(), deadline, output.decode(errors="replace"))
                        if select.select([master], [], [], 0.05)[0]:
                            output += os.read(master, 65536)
                    os.write(master, b"m\x1b[B\x1b[B\x1b[B\r")
                    deadline = time.monotonic() + 5
                    while not marker.exists():
                        self.assertLess(time.monotonic(), deadline, output.decode(errors="replace"))
                        if select.select([master], [], [], 0.05)[0]:
                            output += os.read(master, 65536)
                    pid, staged = json.loads(marker.read_text())
                    started = time.monotonic()
                    os.write(master, keys)
                    while process.poll() is None:
                        self.assertLess(time.monotonic() - started, 2, "quit waited for the download")
                        if select.select([master], [], [], 0.02)[0]:
                            try:
                                os.read(master, 65536)
                            except OSError:
                                break
                    self.assertEqual(process.wait(timeout=1), 0)
                    with self.assertRaises(ProcessLookupError):
                        os.kill(pid, 0)
                    self.assertFalse(Path(staged).parent.exists())
                    self.assertEqual(hashlib.sha256(binary.read_bytes()).digest(), original)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
                    os.close(master)


if __name__ == "__main__":
    unittest.main()
