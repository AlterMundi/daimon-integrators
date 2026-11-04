import copy
import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "device_discovery",
    Path(__file__).resolve().parents[1] / "tools/device_discovery.py",
)
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)


def status():
    return {
        "ok": True,
        "snapshot": {
            "at": 100,
            "compact": True,
            "ui_compact": False,
            "observation": {
                "at": 99,
                "camera_present": True,
                "camera_open": False,
                "microphone_present": True,
                "microphone_running": False,
                "complete": True,
                "camera_users": ["private-process"],
            },
            "pipewire_ok": True,
            "pipewire_camera": False,
            "pipewire_microphone": False,
            "managed": {"phase": "capturing", "output": "/private/file"},
            "system": {"at": 99, "cpu_percent": 2.5},
            "remote_activity": {"private": "history"},
        },
    }


class DeviceDiscoveryTests(unittest.TestCase):
    def test_live_status_keeps_signal_recording_and_requested_ui_distinct(self):
        report = discovery.project(status(), "host-test", now=100)
        self.assertEqual(
            [s["state"] for s in report["perceptors"]],
            ["no-use-detected", "no-use-detected"],
        )
        self.assertEqual(report["managed_activity"]["phase"], "capturing")
        control = report["actuators"][0]
        self.assertTrue(control["requested_compact"])
        self.assertFalse(control["observed_compact"])
        self.assertFalse(control["executed"])
        self.assertNotIn("private", str(report))
        self.assertEqual(report["integrator"]["realm_membership"], "not-established")

    def test_stale_future_and_incomplete_never_claim_inactive(self):
        for now in (90, 110):
            report = discovery.project(status(), "host-test", now=now)
            self.assertEqual(
                [s["state"] for s in report["perceptors"]], ["unknown", "unknown"]
            )
            self.assertIsNone(report["system"]["metrics"]["cpu_percent"])
            self.assertIsNone(report["actuators"][0]["observed_compact"])
        value = status()
        value["snapshot"]["pipewire_ok"] = False
        self.assertTrue(
            all(
                s["state"] == "unknown"
                for s in discovery.project(value, "host-test", now=100)["perceptors"]
            )
        )

    def test_positive_signal_beats_incomplete_presence(self):
        value = status()
        value["snapshot"]["observation"]["camera_present"] = False
        value["snapshot"]["observation"]["complete"] = False
        value["snapshot"]["pipewire_camera"] = True
        report = discovery.project(value, "host-test", now=100)
        self.assertEqual(report["perceptors"][0]["state"], "in-use")
        self.assertEqual(report["perceptors"][1]["state"], "unknown")

    def test_invalid_data_and_resource_refuse(self):
        cases = []
        for key, val in (("at", True), ("at", -1), ("camera_open", 1)):
            value = status()
            value["snapshot"]["observation"][key] = val
            cases.append(value)
        value = status()
        value["snapshot"]["system"]["temperature_c"] = float("nan")
        cases.append(value)
        cases.extend(({"ok": False}, {"ok": True, "snapshot": None}))
        for value in cases:
            with self.assertRaises(ValueError):
                discovery.project(copy.deepcopy(value), "host-test", now=100)
        with self.assertRaises(ValueError):
            discovery.project(status(), "https://arbitrary-target/", now=100)
