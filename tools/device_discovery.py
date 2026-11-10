"""One foreground projection of the existing owner-local integrator status."""

import argparse
import json
import math
import re
import subprocess
import time

BINARY = "/usr/local/bin/daimon-integrators"
MAX_BYTES = 65536


def integer(value):
    return type(value) is int and value >= 0


def fresh(at, now):
    return integer(at) and 0 < at <= now + 1 and now - at <= 4


def project(value, resource, now=None):
    """Expose observation, never infer recording, perception or remote grants."""
    now = int(time.time()) if now is None else now
    if re.fullmatch(r"[a-z][a-z0-9-]{0,95}", resource) is None:
        raise ValueError("invalid_resource_reference")
    if not isinstance(value, dict) or value.get("ok") is not True:
        raise ValueError("status_unavailable")
    snapshot = value.get("snapshot")
    if not isinstance(snapshot, dict):
        raise ValueError("invalid_snapshot")
    observation = snapshot.get("observation")
    managed = snapshot.get("managed")
    if not isinstance(observation, dict) or not isinstance(managed, dict):
        raise ValueError("invalid_snapshot")
    fields = (
        "camera_present",
        "camera_open",
        "microphone_present",
        "microphone_running",
        "complete",
    )
    if any(type(observation.get(k)) is not bool for k in fields):
        raise ValueError("invalid_observation")
    if any(
        type(snapshot.get(k)) is not bool
        for k in ("pipewire_ok", "pipewire_camera", "pipewire_microphone", "compact")
    ):
        raise ValueError("invalid_snapshot")
    if (
        snapshot.get("ui_compact") is not None
        and type(snapshot["ui_compact"]) is not bool
    ):
        raise ValueError("invalid_ui_state")
    if not integer(snapshot.get("at")) or not integer(observation.get("at")):
        raise ValueError("invalid_observation_time")
    current = fresh(snapshot["at"], now)
    recent = current and fresh(observation["at"], now)
    sensors = []
    for name, activity_key, pipewire in (
        ("camera", "camera_open", "pipewire_camera"),
        ("microphone", "microphone_running", "pipewire_microphone"),
    ):
        present = observation[name + "_present"]
        active = observation[activity_key] or snapshot[pipewire]
        complete = observation["complete"] and snapshot["pipewire_ok"]
        state = (
            "unknown"
            if not recent
            else "in-use"
            if active
            else "absent"
            if complete and not present
            else "no-use-detected"
            if complete
            else "unknown"
        )
        sensors.append(
            {
                "id": name,
                "resource_ref": resource,
                "state": state,
                "observed_at": observation["at"],
                "fresh": recent,
                "coverage_complete": complete if recent else False,
                "evidence": "device-use-and-pipewire-observation",
                "media_read": False,
                "recording_or_model_perception": "not-inferred",
            }
        )
    system = snapshot.get("system", {})
    if not isinstance(system, dict):
        raise ValueError("invalid_system_status")
    metrics = {}
    system_fresh = current and fresh(system.get("at"), now)
    for name in ("cpu_percent", "temperature_c", "fan_rpm", "cpu_max_mhz"):
        number = system.get(name)
        if number is not None and (
            type(number) not in (int, float) or not math.isfinite(number)
        ):
            raise ValueError("invalid_system_metric")
        metrics[name] = number if system_fresh else None
    phase = managed.get("phase")
    if not isinstance(phase, str) or len(phase) > 64:
        raise ValueError("invalid_managed_status")
    # No process names, output paths, private activity or native history leave here.
    return {
        "schema": "dm.integrator-discovery/v1",
        "evaluated_at": now,
        "resource_ref": resource,
        "resource_evidence": "caller-selected-existing-resource-record",
        "integrator": {
            "state": "observed" if current else "stale",
            "observed_at": snapshot["at"],
            "source": "owner-local-integrator-status",
            "being_membership": "not-implied",
            "realm_membership": "not-established",
        },
        "perceptors": sensors,
        "actuators": [
            {
                "id": "panel-view",
                "commands": [["view", "compact"], ["view", "expanded"]],
                "requested_compact": snapshot["compact"] if current else None,
                "observed_compact": snapshot.get("ui_compact") if current else None,
                "effect": "local-panel-presentation",
                "permission": "existing-owner-control-required",
                "executed": False,
            }
        ],
        "managed_activity": {"phase": phase if current else "unknown"},
        "system": {"fresh": system_fresh, "metrics": metrics},
        "limits": {
            "observation_max_age_seconds": 4,
            "status_calls": 1,
            "status_timeout_seconds": 6,
            "status_max_bytes": MAX_BYTES,
            "model_calls": 0,
            "device_capture_started": False,
            "controls_executed": False,
        },
    }


def foreground(resource, panel_view=None, *, run=subprocess.run, clock=time.monotonic,
               sleep=time.sleep):
    """One owner action and bounded observed return; never retry a control."""
    if re.fullmatch(r"[a-z][a-z0-9-]{0,95}", resource) is None:
        raise ValueError("invalid_resource_reference")
    if panel_view not in (None, "compact", "expanded"):
        raise ValueError("invalid_panel_view")
    deadline = clock() + 24

    def call(arguments):
        remaining = deadline - clock()
        if remaining <= 0:
            raise TimeoutError("foreground_deadline")
        result = run([BINARY, *arguments], capture_output=True,
                     timeout=min(6, remaining), check=True)
        if len(result.stdout) > MAX_BYTES:
            raise ValueError("status_too_large")
        return json.loads(result.stdout)

    report = project(call(["status"]), resource)
    if panel_view is None:
        return report
    desired = panel_view == "compact"
    receipt = dict(schema="dm.integrator-panel-control/v1", requested_view=panel_view,
                   command_state="not-attempted", outcome="not-observed",
                   control_attempts=0, readback_attempts=0)
    report["control"] = receipt
    if report["actuators"][0]["observed_compact"] is desired:
        receipt.update(outcome="already-observed")
        return report
    # Set uncertainty BEFORE external I/O. A timeout is never another send.
    receipt.update(command_state="uncertain", control_attempts=1)
    report["limits"].pop("controls_executed", None)
    report["actuators"][0].pop("executed", None)
    try:
        ack = call(["view", panel_view])
        if (isinstance(ack, dict) and ack.get("ok") is True
                and ack.get("stage") == "view_requested"
                and ack.get("compact") is desired):
            receipt["command_state"] = "acknowledged"
    except (OSError, ValueError, subprocess.SubprocessError):
        pass
    for attempt in range(3):
        if clock() >= deadline:
            break
        receipt["readback_attempts"] += 1
        try:
            observed = project(call(["status"]), resource)
            observed["limits"].pop("controls_executed", None)
            observed["actuators"][0].pop("executed", None)
            observed["control"] = receipt
            report = observed
            if observed["actuators"][0]["observed_compact"] is desired:
                receipt["outcome"] = "observed"
                break
        except (OSError, ValueError, subprocess.SubprocessError):
            pass
        if attempt < 2 and clock() < deadline:
            sleep(min(.2, max(0, deadline-clock())))
    report["limits"].update(control_attempts_max=1, readback_attempts_max=3,
                            foreground_timeout_seconds=24)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--resource", required=True, help="Existing Shared Resources ID"
    )
    parser.add_argument("--panel-view", choices=("compact", "expanded"),
                        help="Explicit existing owner panel control, with observed return")
    args = parser.parse_args()
    # Validate selection before contacting the existing local service.
    if re.fullmatch(r"[a-z][a-z0-9-]{0,95}", args.resource) is None:
        parser.error("invalid resource reference")
    try:
        report = foreground(args.resource, args.panel_view)
    except (OSError, ValueError, subprocess.SubprocessError):
        print(
            json.dumps(
                {
                    "schema": "dm.integrator-discovery/v1",
                    "resource_ref": args.resource,
                    "integrator": {"state": "unavailable"},
                    "perceptors": [],
                    "actuators": [],
                    "reason": "owner-local-status-unavailable-or-rejected",
                }
            )
        )
        return 1
    print(json.dumps(report, allow_nan=False, sort_keys=True))
    return (0 if report.get("control", {}).get("outcome", "observed")
            in ("observed", "already-observed") else 1)


if __name__ == "__main__":
    raise SystemExit(main())
