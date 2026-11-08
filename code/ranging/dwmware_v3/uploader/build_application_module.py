import os
import shlex
import shutil
from glob import glob

Import("env")

env.AppendUnique(CXXFLAGS=["-std=gnu++23", "-fmodules-ts"])

project_dir = env.subst("$PROJECT_DIR")
build_dir = env.subst("$BUILD_DIR")
module_source = os.path.join(project_dir, "src", "a_serial_commands.cpp")
initiator_source = os.path.join(project_dir, "src", "a_sstwr_initiator_uwbsync.cpp")
reporter_source = os.path.join(project_dir, "src", "a_espnow_reporter.cpp")
anchor_source = os.path.join(project_dir, "src", "a_anchor.cpp")
gateway_source = os.path.join(project_dir, "src", "a_gateway.cpp")
firmware_sources = {
    "tag": os.path.join(project_dir, "src", "a_firmware_tag.cpp"),
    "anchor": os.path.join(project_dir, "src", "a_firmware_anchor.cpp"),
    "gateway": os.path.join(project_dir, "src", "a_firmware_gateway.cpp"),
}
module_cache = os.path.join(project_dir, "gcm.cache")
serial_object = os.path.join(build_dir, "serial_commands_module.o")
initiator_object = os.path.join(build_dir, "sstwr_initiator_uwbsync_module.o")
reporter_object = os.path.join(build_dir, "espnow_reporter_module.o")
anchor_object = os.path.join(build_dir, "anchor_module.o")
gateway_object = os.path.join(build_dir, "gateway_module.o")
firmware_object = os.path.join(build_dir, "firmware_module.o")

if os.path.isdir(module_cache):
    shutil.rmtree(module_cache)
for output in (
    serial_object,
    initiator_object,
    reporter_object,
    anchor_object,
    gateway_object,
    firmware_object,
):
    if os.path.exists(output):
        os.remove(output)


def flatten(values):
    for value in values:
        if isinstance(value, (tuple, list)):
            yield from flatten(value)
        else:
            yield value


def define_flags():
    flags = []
    for define in env.get("CPPDEFINES", []):
        if isinstance(define, tuple):
            name, value = define
            flags.append("-D{}={}".format(name, str(value).replace('\\"', '"')))
        else:
            flags.append("-D{}".format(define))
    return flags


def module_command(source, target, command_env):
    include_paths = [
        os.path.join(project_dir, "include"),
        os.path.join(project_dir, "src"),
        *glob(os.path.join(project_dir, "lib", "*")),
    ]
    include_paths.extend(os.fspath(path) for path in command_env.get("CPPPATH", []))
    for library_root in flatten(command_env.get("LIBSOURCE_DIRS", [])):
        include_paths.extend(glob(os.path.join(str(library_root), "*")))
        include_paths.extend(glob(os.path.join(str(library_root), "*", "src")))
    include_flags = ["-I" + path for path in dict.fromkeys(include_paths)]

    module_flags = []
    skip_param_value = False
    for flag in flatten(command_env.get("CCFLAGS", [])):
        flag = str(flag)
        if skip_param_value:
            skip_param_value = False
            continue
        if flag.startswith("--define-macro="):
            module_flags.append("-D" + flag.split("=", 1)[1])
            continue
        if flag == "--param":
            skip_param_value = True
            continue
        if flag.startswith(("-m", "-f", "-O", "--param", "-nostdlib")):
            module_flags.append(flag)

    return [
        command_env.subst("$CXX"),
        "-std=gnu++23",
        "-fmodules-ts",
        *module_flags,
        *flatten(command_env.get("CXXFLAGS", [])),
        *define_flags(),
        *include_flags,
        "-c",
        source,
        "-o",
        target,
    ]


def compile_module(target, source, command_env):
    if str(target[0]).endswith("serial_commands_module.o"):
        if os.path.isdir(module_cache):
            shutil.rmtree(module_cache)

    command = module_command(str(source[0]), str(target[0]), command_env)
    if command_env.Execute(
        " ".join(shlex.quote(str(arg)) for arg in command)
    ) != 0:
        raise RuntimeError("Failed to build application module")


def compile_serial_module(target, source, env):
    compile_module(target, [module_source], env)


def compile_firmware_module(target, source, env):
    environment = env.subst("$PIOENV")
    if environment == "serialrelay":
        role = "gateway"
    elif environment in {"wemosanchor01"}:
        role = "anchor"
    else:
        role = "tag"
    compile_module(target, [firmware_sources[role]], env)


def compile_initiator_module(target, source, env):
    compile_module(target, [initiator_source], env)


def compile_reporter_module(target, source, env):
    compile_module(target, [reporter_source], env)


def compile_anchor_module(target, source, env):
    compile_module(target, [anchor_source], env)


def compile_gateway_module(target, source, env):
    compile_module(target, [gateway_source], env)


serial_node = env.Command(
    serial_object,
    [],
    compile_serial_module,
)
module_nodes = [serial_node]
is_anchor_environment = env.subst("$PIOENV") == "wemosanchor01"
is_gateway_environment = env.subst("$PIOENV") == "serialrelay"
if is_anchor_environment:
    initiator_node = env.Command(initiator_object, [serial_node], compile_initiator_module)
    reporter_node = env.Command(reporter_object, [initiator_node], compile_reporter_module)
    anchor_node = env.Command(anchor_object, [serial_node, initiator_node, reporter_node], compile_anchor_module)
    module_nodes.extend([initiator_node, reporter_node, anchor_node])
if is_gateway_environment:
    initiator_node = env.Command(initiator_object, [serial_node], compile_initiator_module)
    reporter_node = env.Command(reporter_object, [initiator_node], compile_reporter_module)
    gateway_node = env.Command(gateway_object, [serial_node, reporter_node], compile_gateway_module)
    module_nodes.extend([initiator_node, reporter_node, gateway_node])

firmware_node = env.Command(
    firmware_object,
    module_nodes,
    compile_firmware_module,
)
module_nodes.append(firmware_node)

env.AppendUnique(LIBS=module_nodes)
pio_environment = env.subst("$PIOENV")
tag_environments = {"wemostag01", "wemostag02", "nrftag", "nrftag1", "nrftag2dwm3001c", "nrftag3001c", "nrftag2"}
entry_source = "main_tag.cpp" if pio_environment in tag_environments else "main.cpp"
env.Depends(
    env.File(os.path.join(build_dir, "src", entry_source + ".o")),
    firmware_node,
)
