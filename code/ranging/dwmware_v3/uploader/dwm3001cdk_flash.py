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
   program MBR + SoftDevice S140 7.3.0 at 0x0, then (if present) program the
   already-built application firmware at 0x27000.
"""

from os.path import join, isfile

Import("env")

if env.subst("$PIOPLATFORM") == "nordicnrf52":

    SD_HEX = join(env.subst("$PROJECT_DIR"), "tools", "softdevice",
                  "s140_nrf52_7.3.0_softdevice.hex")

    def _commander_script(target, commands, filename):
        script_path = join(env.subst("$BUILD_DIR"), filename)
        with open(script_path, "w") as fp:
            fp.write("\n".join(commands) + "\n")
        return script_path

    def _jlink_loadfile_script(env, source):
        """Replace PlatformIO's raw-binary J-Link script generator.

        ``firmware.hex`` is an Intel HEX file with absolute addresses.  The
        stock nRF52 J-Link builder uses ``loadbin`` plus a board offset, which
        writes the HEX text as binary.  ``loadfile`` consumes the records and
        preserves their 0x27000 application address.
        """
        build_dir = env.subst("$BUILD_DIR")
        script_path = join(build_dir, "upload.jlink")
        with open(script_path, "w") as fp:
            fp.write("\n".join([
                "h",
                'loadfile "%s"' % str(source),
                "r",
                "q",
            ]) + "\n")
        return script_path

    # The PlatformIO J-Link action evaluates this callable immediately before
    # launching JLinkExe, after any normal pre-actions have run.
    env.Replace(__jlink_cmd_script=_jlink_loadfile_script)

    def _write_softdevice_script(target, source, env):
        commands = [
            "h",
            "r",
            'loadfile "%s"' % SD_HEX,
        ]
        # If the app was built already, flash it in the same go. The app hex
        # is linked/addressed at 0x27000, so plain loadfile is enough.
        firmware = join(env.subst("$BUILD_DIR"), "firmware.hex")
        if isfile(firmware):
            commands.append('loadfile "%s"' % firmware)
        commands.extend(["r", "q"])
        _commander_script(target, commands, "flash_softdevice.jlink")

    def _softdevice_flash():
        return env.VerboseAction(
            "$UPLOADER $UPLOADERFLAGS -CommanderScript $BUILD_DIR/flash_softdevice.jlink",
            "Programming SoftDevice S140 7.3.0 (+ app if built) via J-Link",
        )

    env.AddPreAction("flash_softdevice", _write_softdevice_script)
    env.AddCustomTarget("flash_softdevice", None, _softdevice_flash())
