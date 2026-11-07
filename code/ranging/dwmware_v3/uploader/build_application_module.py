import os
import shlex
import shutil
from glob import glob

Import("env")

project_dir = env.subst("$PROJECT_DIR")
build_dir = env.subst("$BUILD_DIR")
module_source = os.path.join(project_dir, "src", "a_serial_commands.cpp")
firmware_source = os.path.join(project_dir, "src", "a_firmware.cpp")
module_cache = os.path.join(project_dir, "gcm.cache")
serial_object = os.path.join(build_dir, "serial_commands_module.o")
firmware_object = os.path.join(build_dir, "firmware_module.o")


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
    compile_module(target, [firmware_source], env)


serial_node = env.Command(
    serial_object,
    [],
    compile_serial_module,
)
module_nodes = [serial_node]

firmware_node = env.Command(
    firmware_object,
    module_nodes,
    compile_firmware_module,
)
module_nodes.append(firmware_node)

env.AppendUnique(LIBS=module_nodes)
env.Depends(
    env.File(os.path.join(build_dir, "src", "main.cpp.o")),
    firmware_node,
)
