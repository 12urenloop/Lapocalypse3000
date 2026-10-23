"""J-Link upload helpers for the DWM3001CDK (nRF52833, SoftDevice S140 7.3.0).

Registered in platformio.ini via ``extra_scripts = uploader/dwm3001cdk_flash.py``.

The board definition (``boards/dwm3001cdk.json``) links the application at
0x27000 (SoftDevice S140 7.3.0 occupies 0x00000..0x26FFF).

What this script does:

1. Custom upload: PlatformIO's built-in jlink runner uses ``loadbin`` which only
   accepts raw binaries, but the artifact here is IntelHex addressed at
   0x27000. We replace it with a J-Link commander script using ``loadfile`` so
   the hex is flashed at its linked addresses.

2. ``pio run -t flash_softdevice``: one-time preparation of a chip that still
   carries the factory Zephyr/Qorvo firmware (which uses no SoftDevice):
   mass erase, program MBR + SoftDevice S140 7.3.0 at 0x0, then (if present)
   program the already-built application firmware at 0x27000.
"""

import os
import shutil
import sys
from os.path import join, isfile

Import("env")

if env.subst("$PIOPLATFORM") == "nordicnrf52":

    BOARD_JLINK_DEVICE = env.BoardConfig().get("debug.jlink_device", "nRF52833_xxAA")
    SD_HEX = join(env.subst("$PROJECT_DIR"), "tools", "softdevice",
                  "s140_nrf52_7.3.0_softdevice.hex")

    JLINK_BIN = "JLink.exe" if sys.platform == "win32" else "JLinkExe"
    JLINK_CMD = shutil.which(JLINK_BIN)
    if JLINK_CMD is None:
        print("Warning: %s not found in PATH (SEGGER J-Link tools required)" % JLINK_CMD)
        JLINK_CMD = JLINK_BIN
    JLINK_ARGS = ["-device", BOARD_JLINK_DEVICE, "-if", "swd", "-speed", "4000",
                  "-autoconnect", "1", "-NoGui", "1"]

    env.Replace(
        UPLOAD_PROTOCOL="custom",
        JLINK_CMD=JLINK_CMD,
        JLINK_ARGS=" ".join(JLINK_ARGS),
    )
    env.Append(
        UPLOADCMD="$JLINK_CMD $JLINK_ARGS -CommanderScript $BUILD_DIR/upload.jlink",
    )

    def _commander_script(target, commands, filename):
        script_path = join(env.subst("$BUILD_DIR"), filename)
        with open(script_path, "w") as fp:
            fp.write("\n".join(commands) + "\n")
        return script_path

    def _write_upload_script(target, source, env):
        # source[0] is the built firmware (IntelHex, app addressed at 0x27000)
        firmware = str(source[0]) if source else join(env.subst("$BUILD_DIR"), "firmware.hex")
        return _commander_script(
            target,
            [
                "h",
                'loadfile "%s"' % firmware,
                "r",
                "q",
            ],
            "upload.jlink",
        )

    env.AddPreAction("upload", _write_upload_script)

    def _write_softdevice_script(target):
        commands = [
            "h",
            "r",
            "eraseall",
            'loadfile "%s"' % SD_HEX,
            "r",
            "q",
        ]
        # If the app was built already, flash it in the same go. The app hex
        # is linked/addressed at 0x27000, so plain loadfile is enough.
        firmware = join(env.subst("$BUILD_DIR"), "firmware.hex")
        if isfile(firmware):
            commands.insert(3, 'loadfile "%s"' % firmware)
        return _commander_script(target, commands, "flash_softdevice.jlink")

    def _softdevice_flash():
        return env.VerboseAction(
            "$JLINK_CMD $JLINK_ARGS -CommanderScript $BUILD_DIR/flash_softdevice.jlink",
            "Programming SoftDevice S140 7.3.0 (+ app if built) via J-Link",
        )

    env.AddPreAction("flash_softdevice", _write_softdevice_script)
    env.AddCustomTarget("flash_softdevice", None, _softdevice_flash())
