#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate `crates/msi-acpi/src/models/msi_ec.rs` from the Linux msi-ec driver.

msi-ec (https://github.com/BeardOverflow/msi-ec) keeps one `struct msi_ec_conf` per family of MSI
EC firmware: the EC registers of every feature plus the firmware strings (`ALLOWED_FW_*`) the
config applies to. This script turns every config registered in msi-ec's `CONFIGURATIONS[]` into a
Rust `ModelConf` value (types live in `crates/msi-acpi/src/models/mod.rs`).

Usage (run from anywhere; the output path is resolved relative to this script):

    python tools/gen-models.py                 # latest commit on msi-ec's main branch
    python tools/gen-models.py --rev <ref>     # a specific msi-ec commit, tag or branch
    python tools/gen-models.py --source <dir> --rev <sha>
        # offline: <dir> holds msi-ec.c and ec_memory_configuration.h of commit <sha>

The output depends only on the msi-ec sources and the commit SHA, so regenerating from the same
commit reproduces the checked-in file byte for byte. Python 3.9+, standard library only. The
output is laid out the way rustfmt formats it with the repository's rustfmt.toml.

Mapping rules:
- `MSI_EC_ADDR_*` sentinels from ec_memory_configuration.h (values above 0xFF: "unknown" and
  "unsupported" address) become `None`; a feature whose address is a sentinel becomes `None`.
- `leds` and `kbd_backlight` become `None` when none of their addresses is known.
- Trailing comments next to firmware strings (model names) are kept as `//` comments.
- A field missing from a supported feature is an error rather than C's zero default, so a
  half-written upstream entry can never turn into a write to register 0x00.
- Fields the Rust types do not model are listed on stderr and skipped.
"""

from __future__ import annotations

import argparse
import re
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path

REPO = "BeardOverflow/msi-ec"
SOURCE_FILES = ("msi-ec.c", "ec_memory_configuration.h")
OUTPUT = Path(__file__).resolve().parent.parent / "crates/msi-acpi/src/models/msi_ec.rs"

MAX_WIDTH = 110  # rustfmt.toml `max_width`
INDENT = "    "


class GenError(Exception):
    pass


# ----------------------------------------------------------------------------------------------
# Fetching


def http_get(url: str, accept: str | None = None) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "open-msi-control gen-models"})
    if accept:
        request.add_header("Accept", accept)
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8")


def resolve_sha(rev: str) -> str:
    sha = http_get(
        f"https://api.github.com/repos/{REPO}/commits/{rev}", accept="application/vnd.github.sha"
    ).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise GenError(f"GitHub did not return a commit SHA for {rev!r}: {sha[:200]!r}")
    return sha


def fetch_sources(sha: str) -> dict[str, str]:
    return {
        name: http_get(f"https://raw.githubusercontent.com/{REPO}/{sha}/{name}")
        for name in SOURCE_FILES
    }


def read_sources(directory: Path) -> dict[str, str]:
    return {name: (directory / name).read_text(encoding="utf-8") for name in SOURCE_FILES}


# ----------------------------------------------------------------------------------------------
# C tokenizer and designated-initializer parser


@dataclass(frozen=True)
class Token:
    kind: str  # "str", "num", "id", "punct", "comment", "other"
    text: str
    line: int


TOKEN_RE = re.compile(
    r"""
      (?P<ws>\s+)
    | (?P<comment>//[^\n]*|/\*.*?\*/)
    | (?P<str>"(?:[^"\\\n]|\\.)*")
    | (?P<num>0[xX][0-9a-fA-F]+|\d+)
    | (?P<id>[A-Za-z_]\w*)
    | (?P<punct>[{}.,=&])
    | (?P<other>.)
    """,
    re.S | re.X,
)


def tokenize(text: str, first_line: int = 1) -> list[Token]:
    tokens = []
    line = first_line
    for match in TOKEN_RE.finditer(text):
        kind = match.lastgroup
        assert kind is not None
        if kind != "ws":
            tokens.append(Token(kind, match.group(), line))
        line += match.group().count("\n")
    return tokens


def c_string(token: Token) -> str:
    value = token.text[1:-1]
    if "\\" in value:
        raise GenError(f"line {token.line}: escape sequences are not supported: {token.text}")
    return value


class Parser:
    """Parses `{ .field = value, ... }` / `{ value, ... }` into dicts and lists."""

    def __init__(self, tokens: list[Token]):
        self.tokens = [t for t in tokens if t.kind != "comment"]
        self.pos = 0

    def peek(self) -> Token:
        if self.pos >= len(self.tokens):
            raise GenError("unexpected end of initializer")
        return self.tokens[self.pos]

    def take(self, text: str | None = None) -> Token:
        token = self.peek()
        if text is not None and token.text != text:
            raise GenError(f"line {token.line}: expected {text!r}, found {token.text!r}")
        self.pos += 1
        return token

    def value(self):
        token = self.peek()
        if token.text == "{":
            return self.initializer()
        if token.kind in ("num", "id", "str"):
            return self.take()
        raise GenError(f"line {token.line}: unexpected {token.text!r} in initializer")

    def initializer(self):
        self.take("{")
        fields: dict[str, object] = {}
        items: list[object] = []
        while self.peek().text != "}":
            if self.peek().text == ".":
                self.take(".")
                name = self.take()
                if name.kind != "id":
                    raise GenError(f"line {name.line}: expected a field name, found {name.text!r}")
                self.take("=")
                if name.text in fields:
                    raise GenError(f"line {name.line}: field .{name.text} set twice")
                fields[name.text] = self.value()
            else:
                items.append(self.value())
            if self.peek().text == ",":
                self.take(",")
            elif self.peek().text != "}":
                token = self.peek()
                raise GenError(f"line {token.line}: expected ',' or '}}', found {token.text!r}")
        self.take("}")
        if fields and items:
            raise GenError("mixed designated and positional initializers are not supported")
        return items if items or not fields else fields


def braced_body(src: str, open_brace: int) -> tuple[str, int]:
    """Returns the text from `open_brace` through its matching `}`, plus the line it starts on."""
    depth = 0
    first_line = src.count("\n", 0, open_brace) + 1
    for match in TOKEN_RE.finditer(src, open_brace):
        text = match.group()
        if match.lastgroup == "punct" and text == "{":
            depth += 1
        elif match.lastgroup == "punct" and text == "}":
            depth -= 1
            if depth == 0:
                return src[open_brace : match.end()], first_line
    raise GenError(f"unbalanced braces from line {first_line}")


# ----------------------------------------------------------------------------------------------
# msi-ec model


@dataclass
class Firmware:
    version: str
    comment: str | None


@dataclass
class Source:
    sha: str
    string_defines: dict[str, str]
    sentinels: set[int]
    numeric_defines: dict[str, int]
    firmware_lists: dict[str, list[Firmware]]
    confs: dict[str, dict]
    registered: list[str]
    copyright: list[str]
    authors: list[str]


def parse_defines(text: str) -> tuple[dict[str, str], dict[str, int]]:
    strings = {}
    numbers = {}
    for name, value in re.findall(r"^[ \t]*#define[ \t]+(\w+)[ \t]+(\S+)", text, re.M):
        if re.fullmatch(r'"[^"\\]*"', value):
            strings[name] = value[1:-1]
        elif re.fullmatch(r"0[xX][0-9a-fA-F]+|\d+", value):
            numbers[name] = int(value, 0)
    return strings, numbers


def parse_firmware_lists(src: str) -> dict[str, list[Firmware]]:
    lists = {}
    pattern = re.compile(
        r"static\s+const\s+char\s*\*\s*(ALLOWED_FW_\w+)\s*\[\s*\]\s*__initconst\s*=\s*\{"
    )
    for match in pattern.finditer(src):
        body, first_line = braced_body(src, match.end() - 1)
        entries: list[Firmware] = []
        terminated = False
        last_line = None
        for token in tokenize(body, first_line):
            if token.kind == "str":
                if terminated:
                    raise GenError(f"line {token.line}: firmware after NULL in {match.group(1)}")
                version = c_string(token)
                if not re.fullmatch(r"[\x21-\x7e]+", version):
                    raise GenError(f"line {token.line}: unexpected firmware string {token.text}")
                entries.append(Firmware(version, None))
                last_line = token.line
            elif token.kind == "comment" and token.text.startswith("//"):
                if entries and token.line == last_line:
                    entries[-1].comment = " ".join(token.text[2:].split()) or None
            elif token.kind == "id" and token.text == "NULL":
                terminated = True
            elif token.kind == "comment" or token.text in ("{", "}", ","):
                pass
            else:
                raise GenError(f"line {token.line}: unexpected {token.text!r} in {match.group(1)}")
        if not terminated:
            raise GenError(f"{match.group(1)} is not NULL-terminated")
        if match.group(1) in lists:
            raise GenError(f"{match.group(1)} defined twice")
        lists[match.group(1)] = entries
    return lists


def parse_confs(src: str) -> dict[str, dict]:
    confs = {}
    pattern = re.compile(r"static\s+struct\s+msi_ec_conf\s+(CONF_\w+)\s*__initdata\s*=\s*\{")
    for match in pattern.finditer(src):
        body, first_line = braced_body(src, match.end() - 1)
        parser = Parser(tokenize(body, first_line))
        conf = parser.initializer()
        if not isinstance(conf, dict):
            raise GenError(f"{match.group(1)} is not a designated initializer")
        if match.group(1) in confs:
            raise GenError(f"{match.group(1)} defined twice")
        confs[match.group(1)] = conf
    return confs


def parse_registered(src: str) -> list[str]:
    match = re.search(
        r"static\s+struct\s+msi_ec_conf\s*\*\s*CONFIGURATIONS\s*\[\s*\]\s*__initdata\s*=\s*\{", src
    )
    if not match:
        raise GenError("CONFIGURATIONS[] not found")
    body, first_line = braced_body(src, match.end() - 1)
    names = []
    tokens = [t for t in tokenize(body, first_line) if t.kind != "comment"]
    for i, token in enumerate(tokens):
        if token.kind == "id" and token.text != "NULL":
            if i == 0 or tokens[i - 1].text != "&":
                raise GenError(f"line {token.line}: unexpected {token.text!r} in CONFIGURATIONS[]")
            names.append(token.text)
    return names


def parse_source(sources: dict[str, str], sha: str) -> Source:
    src = sources["msi-ec.c"]
    header = sources["ec_memory_configuration.h"]
    header_strings, header_numbers = parse_defines(header)
    src_strings, src_numbers = parse_defines(src)
    sentinels = {
        value
        for name, value in header_numbers.items()
        if name.startswith("MSI_EC_ADDR_") and value > 0xFF
    }
    if not sentinels:
        raise GenError("no MSI_EC_ADDR_* sentinels found in ec_memory_configuration.h")
    # The licence comment at the top of msi-ec.c, kept verbatim if it ever names copyright holders.
    header_comment = re.match(r"\s*(?://[^\n]*\s*)*(?:/\*.*?\*/)?", src, re.S).group(0)
    copyright_lines = [
        " ".join(line.strip(" */").split())
        for line in header_comment.splitlines()
        if re.search(r"(?i)\bcopyright\b|\(c\)|©", line)
    ]
    return Source(
        sha=sha,
        string_defines={**header_strings, **src_strings},
        sentinels=sentinels,
        numeric_defines={**header_numbers, **src_numbers},
        firmware_lists=parse_firmware_lists(src),
        confs=parse_confs(src),
        registered=parse_registered(src),
        copyright=copyright_lines,
        authors=re.findall(r'^MODULE_AUTHOR\("([^"]+)"\);', src, re.M),
    )


# ----------------------------------------------------------------------------------------------
# Conversion to Rust values


class Lit(str):
    """Rust source of a literal."""


@dataclass
class Struct:
    name: str
    fields: list[tuple[str, object]]


@dataclass
class Some:
    value: object


@dataclass
class Array:
    items: list[object]
    comments: list[str | None] | None = None  # trailing `//` comment per item


@dataclass
class Tuple:
    items: list[object]


NONE = Lit("None")


def hex8(value: int) -> Lit:
    return Lit(f"0x{value:02X}")


def rust_str(value: str) -> Lit:
    return Lit('"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"')


def opt_hex(value: int | None) -> object:
    return NONE if value is None else Some(hex8(value))


class Section:
    """One designated initializer of a config (e.g. `.webcam = { ... }`), read with validation."""

    def __init__(self, source: Source, where: str, values: dict):
        self.source = source
        self.where = where
        self.values = values

    def fail(self, message: str):
        raise GenError(f"{self.where}: {message}")

    def raw(self, key: str):
        if key not in self.values:
            self.fail(f"missing .{key}")
        return self.values[key]

    def to_number(self, token, what: str) -> int:
        if isinstance(token, Token):
            if token.kind == "num":
                return int(token.text, 0)
            if token.kind == "id" and token.text in self.source.numeric_defines:
                return self.source.numeric_defines[token.text]
        self.fail(f"{what}: expected a number")

    def to_u8(self, token, what: str) -> int:
        value = self.to_number(token, what)
        if not 0 <= value <= 0xFF:
            self.fail(f"{what}: {value:#x} does not fit in a byte")
        return value

    def address(self, key: str) -> int | None:
        value = self.to_number(self.raw(key), f".{key}")
        if value in self.source.sentinels:
            return None
        if not 0 <= value <= 0xFF:
            self.fail(f".{key}: {value:#x} is neither a byte nor an MSI_EC_ADDR_* sentinel")
        return value

    def u8(self, key: str) -> int:
        return self.to_u8(self.raw(key), f".{key}")

    def bit(self, key: str) -> int:
        value = self.to_number(self.raw(key), f".{key}")
        if not 0 <= value <= 7:
            self.fail(f".{key}: bit {value} out of range")
        return value

    def boolean(self, key: str) -> bool:
        token = self.raw(key)
        if isinstance(token, Token) and token.kind == "id" and token.text in ("true", "false"):
            return token.text == "true"
        self.fail(f".{key}: expected true or false")

    def u8_list(self, key: str) -> list[int]:
        values = self.raw(key)
        if not isinstance(values, list):
            self.fail(f".{key}: expected a list")
        return [self.to_u8(value, f".{key}") for value in values]

    def modes(self, key: str) -> Array:
        """`{ { NAME, value }, ..., MSI_EC_MODE_NULL }` as `("name", value)` tuples."""
        entries = self.raw(key)
        if not isinstance(entries, list):
            self.fail(f".{key}: expected a list of modes")
        items = []
        terminated = False
        for entry in entries:
            if isinstance(entry, Token) and entry.text == "MSI_EC_MODE_NULL":
                terminated = True
                continue
            is_pair = isinstance(entry, list) and len(entry) == 2
            if not (is_pair and all(isinstance(t, Token) for t in entry)):
                self.fail(f".{key}: expected {{ name, value }} entries")
            name_token, value_token = entry
            if name_token.text == "NULL":
                terminated = True
                continue
            if terminated:
                self.fail(f".{key}: mode after the NULL terminator")
            if name_token.kind == "str":
                name = c_string(name_token)
            elif name_token.kind == "id" and name_token.text in self.source.string_defines:
                name = self.source.string_defines[name_token.text]
            else:
                self.fail(f".{key}: unknown mode name {name_token.text!r}")
            if not name:
                self.fail(f".{key}: empty mode name")
            items.append(Tuple([rust_str(name), hex8(self.to_u8(value_token, f".{key}"))]))
        if not terminated:
            self.fail(f".{key}: not terminated by MSI_EC_MODE_NULL")
        return Array(items)


class Converter:
    # Fields of `struct msi_ec_conf` ("") and its sub-structs that the Rust types model.
    KNOWN = {
        "": {
            "allowed_fw", "charge_control_address", "webcam", "fn_win_swap", "cooler_boost",
            "shift_mode", "super_battery", "fan_mode", "cpu", "gpu", "leds", "kbd_bl",
        },
        "webcam": {"address", "block_address", "bit"},
        "fn_win_swap": {"address", "bit", "invert"},
        "cooler_boost": {"address", "bit"},
        "shift_mode": {"address", "modes"},
        "super_battery": {"address", "mask"},
        "fan_mode": {"address", "modes"},
        "cpu": {"rt_temp_address", "rt_fan_speed_address"},
        "gpu": {"rt_temp_address", "rt_fan_speed_address"},
        "leds": {"micmute_led_address", "mute_led_address", "bit"},
        "kbd_bl": {
            "bl_mode_address", "bl_modes", "max_mode", "bl_state_address", "state_base_value",
            "max_state",
        },
    }

    def __init__(self, source: Source):
        self.source = source
        self.skipped: dict[str, list[str]] = {}  # field path -> configs that set it
        self.used_types: set[str] = {"ModelConf"}
        self.firmware_count = 0

    def use(self, type_name: str, fields: list[tuple[str, object]]) -> Struct:
        self.used_types.add(type_name)
        return Struct(type_name, fields)

    def skip_unknown(self, conf_name: str, prefix: str, values: dict, known: set[str]):
        for field in sorted(set(values) - known):
            self.skipped.setdefault(prefix + field, []).append(conf_name)

    def section(self, conf_name: str, conf: dict, key: str) -> Section:
        values = conf.get(key)
        if not isinstance(values, dict):
            raise GenError(f"{conf_name}.{key}: missing or not a designated initializer")
        self.skip_unknown(conf_name, f"{key}.", values, self.KNOWN[key])
        return Section(self.source, f"{conf_name}.{key}", values)

    def conf(self, name: str) -> Struct:
        values = self.source.confs.get(name)
        if values is None:
            raise GenError(f"{name} is listed in CONFIGURATIONS[] but not defined")
        self.skip_unknown(name, "", values, self.KNOWN[""])
        top = Section(self.source, name, values)

        fw_list = top.raw("allowed_fw")
        if not (isinstance(fw_list, Token) and fw_list.text in self.source.firmware_lists):
            top.fail(".allowed_fw: unknown firmware list")
        firmware = self.source.firmware_lists[fw_list.text]
        if not firmware:
            top.fail(f"{fw_list.text} is empty")
        self.firmware_count += len(firmware)
        versions = Array([rust_str(f.version) for f in firmware], [f.comment for f in firmware])

        webcam = self.section(name, values, "webcam")
        camera, block = webcam.address("address"), webcam.address("block_address")
        bit = None if camera is None and block is None else webcam.bit("bit")

        return Struct("ModelConf", [
            ("name", rust_str(name)),
            ("firmware", versions),
            ("charge_control", opt_hex(top.address("charge_control_address"))),
            ("webcam", self.bit_field(camera, bit)),
            ("webcam_block", self.bit_field(block, bit)),
            ("fn_win_swap", self.fn_win_swap(self.section(name, values, "fn_win_swap"))),
            ("cooler_boost", self.cooler_boost(self.section(name, values, "cooler_boost"))),
            ("shift_mode", self.modes(self.section(name, values, "shift_mode"))),
            ("super_battery", self.super_battery(self.section(name, values, "super_battery"))),
            ("fan_mode", self.modes(self.section(name, values, "fan_mode"))),
            ("cpu", self.sensors(self.section(name, values, "cpu"))),
            ("gpu", self.sensors(self.section(name, values, "gpu"))),
            ("leds", self.leds(self.section(name, values, "leds"))),
            ("kbd_backlight", self.kbd_backlight(self.section(name, values, "kbd_bl"))),
        ])

    # Each feature reads its remaining fields only when its address is known, so fields left out
    # of unsupported features are not errors.

    def bit_field(self, address: int | None, bit: int | None) -> object:
        if address is None:
            return NONE
        return Some(self.use("Bit", [("address", hex8(address)), ("bit", Lit(str(bit)))]))

    def fn_win_swap(self, s: Section) -> object:
        address = s.address("address")
        if address is None:
            return NONE
        return Some(self.use("InvertibleBit", [
            ("address", hex8(address)),
            ("bit", Lit(str(s.bit("bit")))),
            ("invert", Lit("true" if s.boolean("invert") else "false")),
        ]))

    def cooler_boost(self, s: Section) -> object:
        address = s.address("address")
        return NONE if address is None else self.bit_field(address, s.bit("bit"))

    def modes(self, s: Section) -> object:
        address = s.address("address")
        if address is None:
            return NONE
        return Some(self.use("Modes", [("address", hex8(address)), ("modes", s.modes("modes"))]))

    def super_battery(self, s: Section) -> object:
        address = s.address("address")
        if address is None:
            return NONE
        return Some(self.use("Masked", [("address", hex8(address)), ("mask", hex8(s.u8("mask")))]))

    def sensors(self, s: Section) -> object:
        return self.use("Sensors", [
            ("rt_temp", opt_hex(s.address("rt_temp_address"))),
            ("rt_fan_speed", opt_hex(s.address("rt_fan_speed_address"))),
        ])

    def leds(self, s: Section) -> object:
        micmute, mute = s.address("micmute_led_address"), s.address("mute_led_address")
        if micmute is None and mute is None:
            return NONE
        return Some(self.use("Leds", [
            ("micmute", opt_hex(micmute)),
            ("mute", opt_hex(mute)),
            ("bit", Lit(str(s.bit("bit")))),
        ]))

    def kbd_backlight(self, s: Section) -> object:
        mode, state = s.address("bl_mode_address"), s.address("bl_state_address")
        if mode is None and state is None:
            return NONE
        return Some(self.use("KbdBacklight", [
            ("mode_address", opt_hex(mode)),
            ("modes", Array([hex8(value) for value in s.u8_list("bl_modes")])),
            ("max_mode", Lit(str(s.u8("max_mode")))),
            ("state_address", opt_hex(state)),
            ("state_base", hex8(s.u8("state_base_value"))),
            ("max_state", Lit(str(s.u8("max_state")))),
        ]))


# ----------------------------------------------------------------------------------------------
# Rust rendering (rustfmt layout for this subset, `use_small_heuristics = "Max"`)


def flat(value) -> str | None:
    """Single-line rendering, or None if the value carries comments."""
    if isinstance(value, Lit):
        return str(value)
    if isinstance(value, Some):
        inner = flat(value.value)
        return None if inner is None else f"Some({inner})"
    if isinstance(value, Tuple):
        parts = [flat(item) for item in value.items]
        return None if None in parts else "(" + ", ".join(parts) + ")"
    if isinstance(value, Array):
        if value.comments and any(value.comments):
            return None
        parts = [flat(item) for item in value.items]
        return None if None in parts else "&[" + ", ".join(parts) + "]"
    if isinstance(value, Struct):
        parts = []
        for name, field in value.fields:
            text = flat(field)
            if text is None:
                return None
            parts.append(f"{name}: {text}")
        return f"{value.name} {{ " + ", ".join(parts) + " }"
    raise TypeError(value)


def render(value, indent: str, prefix: str, suffix: str) -> list[str]:
    """Lines for `prefix value suffix`, the first line starting at `indent`."""
    single = flat(value)
    if single is not None and len(indent) + len(prefix) + len(single) + len(suffix) <= MAX_WIDTH:
        return [f"{indent}{prefix}{single}{suffix}"]
    inner = indent + INDENT
    if isinstance(value, Some):
        return render(value.value, indent, prefix + "Some(", ")" + suffix)
    if isinstance(value, Struct):
        lines = [f"{indent}{prefix}{value.name} {{"]
        for name, field in value.fields:
            lines += render(field, inner, f"{name}: ", ",")
        return lines + [f"{indent}}}{suffix}"]
    if isinstance(value, Array):
        lines = [f"{indent}{prefix}&["]
        for item, comment in zip(value.items, value.comments or [None] * len(value.items)):
            item_lines = render(item, inner, "", ",")
            if comment:
                item_lines[-1] += f" // {comment}"
            lines += item_lines
        return lines + [f"{indent}]{suffix}"]
    raise GenError(f"cannot fit {single!r} into {MAX_WIDTH} columns")


def generate(source: Source) -> tuple[str, Converter]:
    defined = set(source.confs)
    unregistered = sorted(defined - set(source.registered))
    if unregistered:
        print(f"note: not in CONFIGURATIONS[], skipped: {', '.join(unregistered)}", file=sys.stderr)

    converter = Converter(source)
    confs = [converter.conf(name) for name in source.registered]

    lines = [
        "// SPDX-License-Identifier: GPL-2.0-or-later",
        "//",
        f"// Generated by tools/gen-models.py from {REPO}@{source.sha} — do not edit.",
        "//",
        f"// Register tables from msi-ec.c (https://github.com/{REPO}), GPL-2.0-or-later.",
    ]
    if source.copyright:
        lines += [f"// {line}" for line in source.copyright]
    if source.authors:
        lines.append("// msi-ec.c authors (MODULE_AUTHOR):")
        lines += [f"//   {author}" for author in source.authors]
    lines += [
        "",
        "use super::{" + ", ".join(sorted(converter.used_types)) + "};",
        "",
        "pub(super) static CONFS: &[ModelConf] = &[",
    ]
    for conf in confs:
        lines += render(conf, INDENT, "", ",")
    lines.append("];")
    return "\n".join(lines) + "\n", converter


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--rev", default="main", help="msi-ec commit, tag or branch (default: main)")
    parser.add_argument("--source", type=Path, help="offline: directory with the msi-ec sources")
    parser.add_argument("--output", type=Path, default=OUTPUT, help=f"default: {OUTPUT}")
    args = parser.parse_args()

    try:
        if args.source:
            if not re.fullmatch(r"[0-9a-f]{40}", args.rev):
                raise GenError("--source needs --rev set to the full commit SHA of those files")
            sha = args.rev
            sources = read_sources(args.source)
        else:
            sha = resolve_sha(args.rev)
            sources = fetch_sources(sha)
        source = parse_source(sources, sha)
        text, converter = generate(source)
    except (GenError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(text, encoding="utf-8", newline="\n")
    for field, confs in sorted(converter.skipped.items()):
        print(f"skipped field .{field} (not modelled) in: {', '.join(confs)}", file=sys.stderr)
    print(
        f"{len(source.registered)} configs, {converter.firmware_count} firmware strings"
        f" from {REPO}@{sha}",
        file=sys.stderr,
    )
    print(f"wrote {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
