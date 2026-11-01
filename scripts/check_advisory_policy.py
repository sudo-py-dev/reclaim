"""Enforce expiry and exact locked versions for reviewed maintenance notices.

cargo-deny 0.20.2 supports reasons but does not enforce exception expiration.
"""
from datetime import date, datetime, timezone
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REVIEWED = {
    "RUSTSEC-2026-0370": ("zvariant", "5.15.1"),
    "RUSTSEC-2026-0371": ("zbus_names", "4.3.5"),
    "RUSTSEC-2026-0372": ("zvariant_derive", "5.15.1"),
}


def validate(config, packages, today):
    for exception in config["advisories"].get("ignore", []):
        if not isinstance(exception, dict) or exception.get("id") not in REVIEWED:
            raise ValueError("Only the three reviewed maintenance notices may be excepted")
        match = re.match(r"Expires (\d{4}-\d{2}-\d{2})\. ", exception.get("reason", ""))
        if not match or today >= date.fromisoformat(match[1]):
            raise ValueError(f"Expired or undated advisory exception: {exception['id']}")
        name, version = REVIEWED[exception["id"]]
        versions = {p["version"] for p in packages if p["name"] == name}
        if versions != {version}:
            raise ValueError(f"Re-review {exception['id']}: expected {name} {version}, got {versions}")


if __name__ == "__main__":
    config = tomllib.loads((ROOT / "deny.toml").read_text())
    packages = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    validate(config, packages, datetime.now(timezone.utc).date())
    print("Reviewed maintenance exceptions are in date and match locked versions")
