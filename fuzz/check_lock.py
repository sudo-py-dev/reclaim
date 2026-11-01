"""Reject fuzz dependency drift from the production lockfile."""
from pathlib import Path
import tomllib

root = Path(__file__).resolve().parent
production = tomllib.loads((root.parent / "Cargo.lock").read_text())["package"]
fuzz = []
for lock in [root / "Cargo.lock", root / "containers" / "Cargo.lock"]:
    fuzz.extend(tomllib.loads(lock.read_text())["package"])

def identity(package):
    return tuple(package.get(key) for key in ("name", "version", "source", "checksum"))

expected = {identity(package) for package in production}
harness_only = {"reclaim-fuzz", "reclaim-container-fuzz", "arbitrary", "libfuzzer-sys"}
drift = [
    identity(package) for package in fuzz
    if package["name"] not in harness_only and identity(package) not in expected
]
if drift:
    raise SystemExit(f"Fuzz dependencies differ from production Cargo.lock: {drift}")
print("Fuzz dependencies match production Cargo.lock")
