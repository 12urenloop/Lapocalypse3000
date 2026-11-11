import os
import shlex
import shutil
from glob import glob

Import("env")

env.AppendUnique(CXXFLAGS=["-std=gnu++23", "-fmodules-ts"])

project_dir = env.subst("$PROJECT_DIR")
build_dir = env.subst("$BUILD_DIR")
module_cache = os.path.join(project_dir, "gcm.cache")


def get_option(name, default=""):
    try:
        return env.GetProjectOption(name)
    except Exception:
        return default


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
    command = module_command(str(source[0]), str(target[0]), command_env)
    if command_env.Execute(
        " ".join(shlex.quote(str(arg)) for arg in command)
    ) != 0:
        raise RuntimeError("Failed to build C++ module: {}".format(source[0]))


def resolve_path(path):
    return path if os.path.isabs(path) else os.path.join(project_dir, path)


def parse_units(raw):
    units = []
    for item in raw.replace("\n", ";").split(";"):
        item = item.strip()
        if not item:
            continue
        parts = [part.strip() for part in item.split(":")]
        name = parts[0]
        source = parts[1] if len(parts) > 1 else ""
        deps = [dep.strip() for dep in parts[2].split(",")] if len(parts) > 2 and parts[2].strip() else []
        units.append((name, source, deps))
    return units


firmware_source = get_option("custom_module_firmware", "").strip()
if not firmware_source:
    Return()

unit_specs = parse_units(get_option("custom_module_units", ""))
entry_source = get_option("custom_module_entry", "").strip()

# Reset module BMI cache and previous module objects for a clean, ordered build.
if os.path.isdir(module_cache):
    shutil.rmtree(module_cache)

objects = [os.path.join(build_dir, "{}_module.o".format(name)) for name, _, _ in unit_specs]
firmware_object = os.path.join(build_dir, "firmware_module.o")
for output in [*objects, firmware_object]:
    if os.path.exists(output):
        os.remove(output)

nodes = {}
for name, source, deps in unit_specs:
    dep_nodes = [nodes[dep] for dep in deps]
    target = os.path.join(build_dir, "{}_module.o".format(name))
    src = resolve_path(source)
    nodes[name] = env.Command(
        target,
        dep_nodes,
        lambda target, source, env, src=src: compile_module(target, [src], env),
    )

firmware_node = env.Command(
    firmware_object,
    list(nodes.values()),
    lambda target, source, env: compile_module(
        target, [resolve_path(firmware_source)], env
    ),
)

module_nodes = [*nodes.values(), firmware_node]
env.AppendUnique(LIBS=module_nodes)

if entry_source:
    env.Depends(
        env.File(os.path.join(build_dir, "src", entry_source + ".o")),
        firmware_node,
    )
