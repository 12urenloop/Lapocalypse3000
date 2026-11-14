#!/usr/bin/env python3
import argparse
import json
import os
import re
import shlex
from glob import glob

PROJECT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

IMPORT_RE = re.compile(r"(?:export\s+)?import\s+([A-Za-z0-9_.:]+)\s*;")
EXPORT_RE = re.compile(r"export\s+module\s+([A-Za-z0-9_.:]+)\s*;")
INCLUDE_RE = re.compile(r'^\s*#\s*include\s*"([^"]+)"')

GCC_ONLY_FLAGS = {
    "-fmodules-ts",
    "-mlongcalls",
    "-mdisable-hardware-atomics",
    "-fstrict-volatile-bitfields",
    "-fno-tree-switch-conversion",
    "-fno-jump-tables",
    "-fipa-cp-clone",
    "-fgcse-after-reload",
    "-fno-threadsafe-statics",
    "-fstack-clash-protection",
}
GCC_ONLY_PREFIXES = ("--param=",)

CLANGD_MODULE_DIR = os.path.join(".clangd-db", "modules")
CLANGD_TARGET = "thumbv7em-none-eabihf"


def read_text(path):
    with open(path, "r", encoding="utf-8") as handle:
        return handle.read()


def get_env_options(platformio_ini, env_name):
    options = {}
    current = None
    for raw in platformio_ini.splitlines():
        line = raw.strip()
        if not line or line.startswith(";") or line.startswith("#"):
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1].strip()
            current = section[4:] if section.startswith("env:") else None
            continue
        if current == env_name and "=" in raw:
            key, value = raw.split("=", 1)
            options[key.strip()] = value.strip()
    return options


def resolve_path(path):
    return path if os.path.isabs(path) else os.path.join(PROJECT_DIR, path)


def scan_module(path):
    export_name = None
    imports = set()
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            match = EXPORT_RE.match(stripped)
            if match:
                export_name = match.group(1)
                continue
            match = IMPORT_RE.match(stripped)
            if match:
                imports.add(match.group(1))
    if not export_name:
        export_name = os.path.splitext(os.path.basename(path))[0]
    return export_name, imports


def collect_imports(path, seen=None):
    if seen is None:
        seen = set()
    path = os.path.normpath(path)
    if path in seen or not os.path.isfile(path):
        return set()
    seen.add(path)
    imports = set()
    try:
        with open(path, "r", encoding="utf-8") as handle:
            lines = handle.readlines()
    except OSError:
        return imports
    for line in lines:
        stripped = line.strip()
        match = IMPORT_RE.match(stripped)
        if match:
            imports.add(match.group(1))
        inc = INCLUDE_RE.match(line)
        if inc:
            name = inc.group(1)
            candidates = [
                os.path.normpath(os.path.join(os.path.dirname(path), name)),
                os.path.normpath(os.path.join(PROJECT_DIR, "src", name)),
                os.path.normpath(os.path.join(PROJECT_DIR, "include", name)),
            ]
            for candidate in candidates:
                if os.path.isfile(candidate):
                    imports.update(collect_imports(candidate, seen))
                    break
    return imports


def topo_sort(unit_map):
    deps = {name: [imp for imp in info["imports"] if imp in unit_map] for name, info in unit_map.items()}
    indegree = {name: 0 for name in unit_map}
    reverse = {name: [] for name in unit_map}
    for name, reqs in deps.items():
        for req in reqs:
            indegree[name] += 1
            reverse[req].append(name)
    ready = sorted([name for name, deg in indegree.items() if deg == 0])
    order = []
    while ready:
        name = ready.pop(0)
        order.append(name)
        for nxt in sorted(reverse[name]):
            indegree[nxt] -= 1
            if indegree[nxt] == 0:
                ready.append(nxt)
                ready.sort()
    if len(order) != len(unit_map):
        remaining = [name for name in unit_map if name not in order]
        raise RuntimeError("Cyclic or unresolved C++ module dependencies: {}".format(", ".join(sorted(remaining))))
    return order


def discover_all_modules():
    discovered = {}
    for path in sorted(glob(os.path.join(PROJECT_DIR, "src", "**", "*.module.cpp"), recursive=True)):
        mod_name, imports = scan_module(path)
        discovered[mod_name] = {"source": os.path.normpath(path), "imports": imports}
    return discovered


def topo_sort_all(discovered):
    return topo_sort(discovered) if discovered else []


def discover_modules(firmware_source):
    discovered = {}
    for path in sorted(glob(os.path.join(PROJECT_DIR, "src", "**", "*.module.cpp"), recursive=True)):
        mod_name, imports = scan_module(path)
        discovered[mod_name] = {"source": os.path.normpath(path), "imports": imports}
    firmware_path = os.path.normpath(resolve_path(firmware_source))
    firmware_imports = collect_imports(firmware_path)
    needed = set()
    stack = [imp for imp in firmware_imports if imp in discovered]
    while stack:
        name = stack.pop()
        if name in needed:
            continue
        needed.add(name)
        imports = discovered[name]["imports"] | collect_imports(discovered[name]["source"])
        for imp in imports:
            if imp in discovered and imp not in needed:
                stack.append(imp)
    unit_map = {name: discovered[name] for name in needed}
    return unit_map, topo_sort(unit_map)


def norm_rel(path):
    path = os.path.normpath(path)
    try:
        rel = os.path.relpath(path, PROJECT_DIR)
        if not rel.startswith(".."):
            return rel
    except ValueError:
        pass
    return path


def detect_env(entries, explicit_env=None):
    if explicit_env:
        return explicit_env
    for entry in entries:
        m = re.search(r"/\.pio/build/([^/]+)/", entry.get("command", "") + " " + entry.get("file", ""))
        if m:
            return m.group(1)
    raise RuntimeError("Could not detect PlatformIO env from compile_commands.json; pass --env")


def toolchain_isystem_flags(compiler):
    compiler = os.path.realpath(compiler) if os.path.exists(compiler) else compiler
    root = os.path.dirname(os.path.dirname(compiler))  # .../bin/<cc> -> toolchain root
    flags = []
    candidates = []

    # arm-none-eabi-gcc toolchain layout
    m = re.search(r"arm-none-eabi[^/]*$", os.path.basename(compiler))
    if m:
        cxx_base = os.path.join(root, "arm-none-eabi", "include", "c++")
        gcc_base = os.path.join(root, "lib", "gcc", "arm-none-eabi")
        versions = sorted(glob(os.path.join(cxx_base, "*")))
        for ver_dir in versions:
            ver = os.path.basename(ver_dir)
            candidates.append(ver_dir)
            # target/multilib C++ dirs
            candidates.extend(sorted(glob(os.path.join(ver_dir, "arm-none-eabi", "**"), recursive=True)))
            candidates.extend(sorted(glob(os.path.join(gcc_base, ver, "include"))))
            candidates.extend(sorted(glob(os.path.join(gcc_base, ver, "include-fixed"))))
        candidates.append(os.path.join(root, "arm-none-eabi", "include"))
        candidates.append(os.path.join(root, "arm-none-eabi", "include", "newlib-nano"))

    # xtensa-esp32-elf-gcc (ESP32 Arduino) layout
    m = re.search(r"xtensa-.*-elf[^/]*$", os.path.basename(compiler))
    if m and not candidates:
        target = re.sub(r"-g(\+\+|cc)$", "", os.path.basename(compiler))
        cxx_base = os.path.join(root, target, "include", "c++")
        gcc_base = os.path.join(root, "lib", "gcc", target)
        for ver_dir in sorted(glob(os.path.join(cxx_base, "*"))):
            ver = os.path.basename(ver_dir)
            candidates.append(ver_dir)
            candidates.append(os.path.join(ver_dir, target))
            candidates.append(os.path.join(ver_dir, "backward"))
            candidates.append(os.path.join(gcc_base, ver, "include"))
            candidates.append(os.path.join(gcc_base, ver, "include-fixed"))
        candidates.append(os.path.join(root, target, "include"))

    seen = set()
    for path in candidates:
        if not path or not os.path.isdir(path):
            continue
        norm = os.path.normpath(path)
        if norm in seen:
            continue
        seen.add(norm)
        flags.extend(["-isystem", path])
    return flags


def sanitize_argv(argv):
    out = []
    i = 0
    saw_std = False
    is_cxx = any(a.endswith((".cpp", ".cc", ".cxx", ".C")) for a in argv) or os.path.basename(argv[0]).endswith(("g++", "c++")) if argv else False
    while i < len(argv):
        arg = argv[i]
        if arg == "--param":
            i += 2
            continue
        if arg in GCC_ONLY_FLAGS or any(arg.startswith(p) for p in GCC_ONLY_PREFIXES):
            i += 1
            continue
        if arg.startswith("-std=") and "++" in arg:
            if not saw_std:
                out.append("-std=gnu++23")
                saw_std = True
            i += 1
            continue
        out.append(arg)
        i += 1
    if is_cxx and not any(a.startswith("-std=") and "++" in a for a in out):
        out.insert(1 if out else 0, "-std=gnu++23")
    # Add toolchain stdlib headers so clangd can resolve <atomic>, <cstdint>, etc.
    if argv and is_cxx:
        extra = toolchain_isystem_flags(argv[0])
        existing = set(out)
        if extra:
            filtered = []
            j = 0
            while j < len(extra):
                if extra[j] == "-isystem" and j + 1 < len(extra):
                    if extra[j] not in existing or extra[j + 1] not in existing:
                        filtered.extend([extra[j], extra[j + 1]])
                    j += 2
                else:
                    filtered.append(extra[j])
                    j += 1
            out[1:1] = filtered
    return out


def ensure_arg_pair(argv, flag, value):
    if flag in argv:
        return argv
    return [argv[0]] + [flag, value] + argv[1:] if argv else [flag, value]


def add_clangd_module_flags(argv, module_dir_abs, module_name=None, pcm_path=None):
    out = list(argv)
    if "--target" not in " ".join(out):
        out.insert(1 if out else 0, "--target=" + CLANGD_TARGET)
    has_prebuilt = any(a == "-fprebuilt-module-path" or a.startswith("-fprebuilt-module-path=") for a in out)
    if not has_prebuilt:
        out[1:1] = ["-fprebuilt-module-path=" + module_dir_abs]
    if module_name and pcm_path:
        spec = "-fmodule-file={}={}".format(module_name, pcm_path)
        if spec not in out:
            out[1:1] = [spec]
    return out


def sanitize_command(command, module_dir_abs=None):
    try:
        argv = shlex.split(command)
    except ValueError:
        argv = command.split()
    argv = sanitize_argv(argv)
    # Do not inject or preserve C++ module flags in arbitrary compile commands:
    # adding -fprebuilt-module-path/-fmodule-file to non-module TUs can make
    # clangd load or index with module state/PCMs in contexts they were not built
    # for and crash. Module flags are added explicitly for the selected
    # firmware/module/entry sources.
    cleaned = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "-fprebuilt-module-path":
            i += 2
            continue
        if a.startswith("-fprebuilt-module-path=") or a.startswith("-fmodule-file="):
            i += 1
            continue
        cleaned.append(a)
        i += 1
    argv = cleaned
    return " ".join(shlex.quote(a) for a in argv)


def make_command_for_source(template_command, source_path, output_path=None, module_dir_abs=None):
    try:
        argv = shlex.split(template_command)
    except ValueError:
        argv = template_command.split()
    argv = sanitize_argv(argv)
    src = os.path.normpath(source_path)
    out = output_path or os.path.join(PROJECT_DIR, ".pio", "build", "clangd", norm_rel(src).replace(os.sep, "_") + ".o")

    source_exts = (
        ".c", ".cc", ".cpp", ".cxx", ".C", ".c++",
        ".s", ".S", ".asm",
    )

    def is_source_arg(arg):
        # Keep option values/flags; only strip positional source inputs.
        if arg.startswith("-"):
            return False
        if arg.endswith(".module.cpp"):
            return True
        return arg.endswith(source_exts)

    # Replace -c <file> / -o <file> and drop template source inputs.
    new_argv = []
    i = 0
    replaced_c = False
    replaced_o = False
    while i < len(argv):
        arg = argv[i]
        if arg == "-c":
            new_argv.append("-c")
            replaced_c = True
            # Consume an input path after -c if present.
            if i + 1 < len(argv):
                nxt = argv[i + 1]
                if not nxt.startswith("-") or nxt == "-":
                    i += 2
                    continue
            i += 1
            continue
        if arg == "-o" and i + 1 < len(argv):
            new_argv.extend(["-o", out])
            i += 2
            replaced_o = True
            continue
        if is_source_arg(arg):
            i += 1
            continue
        new_argv.append(arg)
        i += 1

    if not replaced_c:
        new_argv.append("-c")
    if not replaced_o:
        new_argv.extend(["-o", out])

    # Ensure the intended source file is compiled.
    if src not in new_argv:
        new_argv.append(src)

    # Ensure language treated as C++ for module files.
    if src.endswith(".module.cpp") and "-xc++" not in new_argv:
        new_argv.insert(1 if new_argv else 0, "-xc++")

    if module_dir_abs:
        new_argv = add_clangd_module_flags(new_argv, module_dir_abs)

    return " ".join(shlex.quote(a) for a in new_argv)


def clang_precompile_command(template_command, module_name, source_path, pcm_path, module_dir_abs, dep_pcm_map):
    try:
        argv = shlex.split(template_command)
    except ValueError:
        argv = template_command.split()
    argv = sanitize_argv(argv)
    src = os.path.normpath(source_path)
    argv[0] = "clang++"

    new_argv = [argv[0], "--target=" + CLANGD_TARGET, "-x", "c++-module", "--precompile", "-o", pcm_path]
    i = 1
    while i < len(argv):
        a = argv[i]
        if a == "-c":
            i += 1
            if i < len(argv) and (not argv[i].startswith("-") or argv[i] == "-"):
                i += 1
            continue
        if a == "-o":
            i += 2
            continue
        if a == "-xc++":
            i += 1
            continue
        if a.startswith("-"):
            new_argv.append(a)
            i += 1
            continue
        # positional source args
        if a.endswith((".c", ".cc", ".cpp", ".cxx", ".C", ".c++", ".s", ".S", ".asm")) or a.endswith(".module.cpp"):
            i += 1
            continue
        new_argv.append(a)
        i += 1

    for dep, dep_pcm in sorted(dep_pcm_map.items()):
        if os.path.isfile(dep_pcm):
            new_argv.extend(["-fmodule-file={}={}".format(dep, dep_pcm)])
    new_argv.append(src)
    return new_argv


def build_clangd_module_pcms(template_command, discovered, order):
    module_dir_abs = os.path.join(PROJECT_DIR, CLANGD_MODULE_DIR)
    os.makedirs(module_dir_abs, exist_ok=True)
    # Remove stale PCMs so clangd does not pick up old or broken artifacts.
    for path in glob(os.path.join(module_dir_abs, "*.pcm")):
        try:
            os.remove(path)
        except OSError:
            pass
    pcm_paths = {name: os.path.join(module_dir_abs, "{}.pcm".format(name)) for name in discovered}
    built = []
    for name in order:
        info = discovered[name]
        dep_pcm_map = {imp: pcm_paths[imp] for imp in info["imports"] if imp in pcm_paths}
        cmd = clang_precompile_command(template_command, name, info["source"], pcm_paths[name], module_dir_abs, dep_pcm_map)
        try:
            import subprocess
            proc = subprocess.run(cmd, cwd=PROJECT_DIR, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        except Exception as e:
            print("Warning: failed to run clang++ for module {}: {}".format(name, e))
            continue
        if proc.returncode == 0 and os.path.isfile(pcm_paths[name]):
            built.append(name)
        else:
            # keep best-effort: report concise failure
            tail = "\n".join((proc.stderr or "").splitlines()[-12:])
            print("Warning: clangd module precompile failed for {} (rc={}):\n{}".format(name, proc.returncode, tail))
    return built, pcm_paths, module_dir_abs


def pick_template(entries, env_name, entry_source):
    preferred = []
    entry_rel = norm_rel(resolve_path(entry_source)) if entry_source else None
    for e in entries:
        f = norm_rel(e.get("file", ""))
        cmd = e.get("command", "")
        if f.endswith((".c", ".S", ".s")):
            continue
        if ".pio/build/{}/".format(env_name) not in cmd and ".pio/build/{}/".format(env_name) not in f:
            continue
        score = 0
        if entry_rel and f == entry_rel:
            score += 100
        if f.startswith("src/"):
            score += 20
        preferred.append((score, e))
    if preferred:
        preferred.sort(key=lambda x: x[0], reverse=True)
        return preferred[0][1].get("command", "")
    # Fallback: any C++ command for env.
    for e in entries:
        cmd = e.get("command", "")
        if ".pio/build/{}/".format(env_name) in cmd and not e.get("file", "").endswith(".c"):
            return cmd
    for e in entries:
        if not e.get("file", "").endswith(".c"):
            return e.get("command", "")
    raise RuntimeError("No C++ compile command template found for env {}".format(env_name))


def main():
    parser = argparse.ArgumentParser(description="Generate a clangd-oriented compile_commands.json for a PlatformIO env with C++ modules.")
    parser.add_argument("--env", help="PlatformIO environment name")
    parser.add_argument("--compiledb", default="compile_commands.json", help="Input PlatformIO compile_commands.json")
    parser.add_argument("--output", default=".clangd-db/compile_commands.json", help="Output clangd compile_commands.json")
    parser.add_argument("--clangd-module-pcms", action="store_true", help="Precompile and attach module PCMs for clangd (can improve module resolution but may crash some clangd versions)")
    parser.add_argument("--no-clangd-module-pcms", action="store_true", help="Do not precompile or attach module PCMs (safer, default)")
    args = parser.parse_args()

    compiledb_path = resolve_path(args.compiledb)
    output_path = resolve_path(args.output)
    with open(compiledb_path, "r", encoding="utf-8") as handle:
        entries = json.load(handle)

    env_name = detect_env(entries, args.env)
    ini = read_text(os.path.join(PROJECT_DIR, "platformio.ini"))
    env_opts = get_env_options(ini, env_name)
    firmware_source = env_opts.get("custom_module_firmware", "").strip()
    entry_source = env_opts.get("custom_module_entry", "").strip()

    module_dir_abs = os.path.join(PROJECT_DIR, CLANGD_MODULE_DIR)
    built_pcms = []
    pcm_paths = {}
    discovered_all = {}
    all_order = []

    unit_map = {}
    order = []
    use_module_pcms = bool(getattr(args, "clangd_module_pcms", False)) and not bool(getattr(args, "no_clangd_module_pcms", False))
    if firmware_source:
        template = pick_template(entries, env_name, entry_source)
        unit_map, order = discover_modules(firmware_source)
        if unit_map and use_module_pcms:
            # Precompile only modules required by the selected firmware for this env.
            # Precompiling every module in the repo with this env's flags can fail for
            # modules that belong to other firmwares/environments.
            built_pcms, pcm_paths, module_dir_abs = build_clangd_module_pcms(template, unit_map, order)
        else:
            # Default/safe: do not precompile/attach C++ module PCMs for clangd.
            # For embedded cross-target C++23 modules, some clangd versions can
            # crash when loading or indexing with prebuilt module state in certain
            # contexts. Remove stale PCMs that could be discovered accidentally.
            if os.path.isdir(module_dir_abs):
                for path in glob(os.path.join(module_dir_abs, "*.pcm")):
                    try:
                        os.remove(path)
                    except OSError:
                        pass
            module_dir_abs = None

    # Base sanitized DB.
    by_file = {}
    for e in entries:
        file_path = norm_rel(e.get("file", ""))
        if not file_path:
            continue
        cmd = e.get("command")
        if not cmd and "arguments" in e:
            cmd = " ".join(shlex.quote(str(a)) for a in e["arguments"])
        if not cmd:
            continue
        by_file[file_path] = {
            "directory": PROJECT_DIR,
            "command": sanitize_command(cmd, module_dir_abs=module_dir_abs),
            "file": file_path,
        }

    module_sources = []
    selected_firmware = None
    if firmware_source:
        selected_firmware = norm_rel(resolve_path(firmware_source))
        template = pick_template(entries, env_name, entry_source)
        # Add/replace required module units and selected firmware.
        # Attach module PCM flags only when explicitly enabled (helps module
        # resolution but can crash some clangd versions with embedded modules).
        cmd_module_dir = module_dir_abs if use_module_pcms and built_pcms else None
        for name in order:
            src = unit_map[name]["source"]
            rel = norm_rel(src)
            by_file[rel] = {
                "directory": PROJECT_DIR,
                "command": make_command_for_source(template, src, module_dir_abs=cmd_module_dir),
                "file": rel,
            }
            module_sources.append(rel)
        by_file[selected_firmware] = {
            "directory": PROJECT_DIR,
            "command": make_command_for_source(template, selected_firmware, module_dir_abs=cmd_module_dir),
            "file": selected_firmware,
        }
        # Ensure entry source present/sanitized for env.
        selected_entry = None
        if entry_source:
            entry_candidates = [
                os.path.join(PROJECT_DIR, "src", entry_source),
                os.path.join(PROJECT_DIR, entry_source),
            ]
            for cand in entry_candidates:
                if os.path.isfile(cand):
                    rel = norm_rel(cand)
                    selected_entry = rel
                    by_file[rel] = {
                        "directory": PROJECT_DIR,
                        "command": make_command_for_source(template, cand, module_dir_abs=cmd_module_dir),
                        "file": rel,
                    }
                    break
            # Provide explicit commands for other project entry points to avoid
            # inference from the selected module firmware. Only the selected entry
            # should see module PCM flags.
            for path in glob(os.path.join(PROJECT_DIR, "src", "main_*.cpp")):
                rel = norm_rel(path)
                if rel in by_file:
                    continue
                by_file[rel] = {
                    "directory": PROJECT_DIR,
                    "command": make_command_for_source(template, path, module_dir_abs=(cmd_module_dir if rel == selected_entry else None)),
                    "file": rel,
                }
    else:
        # Non-module env: just sanitize existing DB.
        pass

    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as handle:
        json.dump(list(by_file.values()), handle, indent=2)
        handle.write("\n")

    print("Wrote clangd compile DB: {}".format(norm_rel(output_path)))
    print("Environment: {}".format(env_name))
    if selected_firmware:
        print("Firmware: {}".format(selected_firmware))
        print("Module units: {}".format(len(module_sources)))
        for rel in module_sources:
            print("  - {}".format(rel))
    if built_pcms:
        print("Prebuilt clangd modules: {} in {}".format(len(built_pcms), norm_rel(module_dir_abs)))
        for name in built_pcms:
            print("  - {}".format(name))


if __name__ == "__main__":
    main()
