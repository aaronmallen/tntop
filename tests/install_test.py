import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest


INSTALLER = Path(__file__).resolve().parents[1] / "assets" / "install.sh"


class InstallTest(unittest.TestCase):
    def run_installer(self, platform="Darwin/arm64", failure=""):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            commands = root / "commands"
            commands.mkdir()
            destination = root / "bin with spaces"
            destination.mkdir()
            binary = destination / "tntop"
            binary.write_text("old")
            target = (
                "aarch64-apple-darwin"
                if platform == "Darwin/arm64"
                else "x86_64-unknown-linux-gnu"
            )
            asset = f"tntop-0.1.0-{target}"
            archive = root / f"{asset}.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                data = b"new"
                entry = tarfile.TarInfo(f"{asset}/tntop")
                entry.size = len(data)
                output.addfile(entry, io.BytesIO(data))
            checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
            if failure == "checksum":
                checksum = "0" * 64
            if failure == "archive":
                archive.write_bytes(b"not an archive")
                checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
            (root / "SHA256SUMS").write_text(f"{checksum}  {archive.name}\n")
            system, machine = platform.split("/")
            (commands / "uname").write_text(
                f'#!/bin/sh\ncase "$1" in -s) echo "{system}";; -m) echo "{machine}";; esac\n'
            )
            (commands / "curl").write_text(
                """#!/bin/sh
case "$*" in
*releases/latest*) echo '{"tag_name":"0.1.0"}'; exit ;;
esac
[ "$FAILURE" != download ] || exit 22
while [ "$#" -gt 0 ]; do
    case "$1" in
    https:*) source=${1##*/} ;;
    -o) shift; destination=$1 ;;
    esac
    shift
done
cp "$FIXTURES/$source" "$destination"
"""
            )
            for command in commands.iterdir():
                command.chmod(0o755)
            result = subprocess.run(
                ["sh", str(INSTALLER), "latest", str(destination)],
                env={
                    **os.environ,
                    "PATH": f"{commands}:{os.environ['PATH']}",
                    "FIXTURES": str(root),
                    "FAILURE": failure,
                },
                capture_output=True,
                text=True,
            )
            if failure or platform == "Linux/aarch64":
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(binary.read_text(), "old")
            else:
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(binary.read_text(), "new")
                self.assertTrue(os.access(binary, os.X_OK))
            self.assertEqual(list(destination.iterdir()), [binary])

    def test_supported_platforms_replace_existing_binary(self):
        for platform in ["Darwin/arm64", "Linux/x86_64"]:
            with self.subTest(platform=platform):
                self.run_installer(platform)

    def test_unsupported_platform_keeps_existing_binary(self):
        self.run_installer("Linux/aarch64")

    def test_failed_download_checksum_or_extraction_keeps_existing_binary(self):
        for failure in ["download", "checksum", "archive"]:
            with self.subTest(failure=failure):
                self.run_installer(failure=failure)


if __name__ == "__main__":
    unittest.main()
