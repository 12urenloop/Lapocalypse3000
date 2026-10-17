"""PlatformIO extra script used by auto_upload_by_device_info.ts.

Registered as a POST extra script (``PLATFORMIO_EXTRA_SCRIPTS=post:<this>``).
It attaches a pre-action to the ``upload`` target which, right before the
uploader would run, resolves the *exact* upload command line that PlatformIO
generated (esptool / nrfutil / ... including the flash offsets that only become
available during the build) and writes it to ``$PIO_FLASH_PLAN_OUT`` as JSON.

The resolved ``argv`` can later be replayed per port to flash already-built
binaries in parallel, without re-running the build.

When ``PIO_FLASH_PLAN_NEUTER=1`` is set the upload command is replaced with a
no-op after capturing, so the same invocation can produce a "flash plan" for an
environment without actually touching any device.  This is what lets the
TypeScript uploader build one variant (and capture the plan) while deferring the
real flashing to a parallel stage.
"""

import json
import os

Import("env")


def _resolve_argv(target, source, env):
    return [
        str(arg) for arg in env.subst_list("$UPLOADCMD", target=target, source=source)[0]
    ]


def _capture(target, source, env):
    out = os.environ.get("PIO_FLASH_PLAN_OUT")
    if not out:
        return

    plan = {
        "argv": _resolve_argv(target, source, env),
        "port": env.subst("$UPLOAD_PORT"),
        "firmware": [str(node) for node in source],
        "uploader": env.get("UPLOADER"),
        "upload_protocol": env.get("UPLOAD_PROTOCOL"),
        "platform": env.subst("$PIOPLATFORM"),
    }

    tmp = out + ".tmp"
    with open(tmp, "w", encoding="utf8") as fp:
        json.dump(plan, fp, indent=2)
        fp.write("\n")
    os.replace(tmp, out)
    print("[pio_flash_plan] wrote %s" % out)

    if os.environ.get("PIO_FLASH_PLAN_NEUTER") == "1":
        # Keep the build side effects (firmware.bin, plan) but do not flash.
        env.Replace(UPLOADCMD="true")


def _describe(target, source, env, executor=None):
    # Replaces the raw ``_capture([...], [...])`` repr that PlatformIO would
    # otherwise print when it echoes the pre-action being run.
    return "Capturing resolved flash plan"


if os.environ.get("PIO_FLASH_PLAN_OUT"):
    env.AddPreAction("upload", env.Action(_capture, strfunction=_describe))
