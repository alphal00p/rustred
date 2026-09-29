from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("convert_native_v5_to_v6", HERE / "convert_native_v5_to_v6.py")
CONV = importlib.util.module_from_spec(spec)
spec.loader.exec_module(CONV)


def state(version=5, user_data=b"\x00", variable_kind=0):
    symbol = (struct.pack("<I", 7) + struct.pack("<I", 1) + b"d" + struct.pack("<I", 4) + b"test"
              + b"\x00" + struct.pack("<I", 0) + struct.pack("<H", 0) + struct.pack("<H", 0)
              + user_data + b"\x00")
    variables = struct.pack("<Q", 1) + bytes([variable_kind]) + struct.pack("<I", 7)
    return (struct.pack("<IHB", 0x37871367, version, 0) + struct.pack("<Q", 1) + symbol
            + struct.pack("<Q", 0) + struct.pack("<Q", 1) + variables)


def frame(fmt=0, atom_type=1, payload=b"\x05\x01"):
    atom = bytes([atom_type]) + payload
    body = bytes([fmt]) + struct.pack("<Q", len(atom)) + atom
    return struct.pack("<Q", len(body)) + body


def program(state_bytes=None, frames=None, extra_tag=None):
    frames = [frame(), frame(payload=b"\x09\x02\x03")] if frames is None else frames
    table = struct.pack("<Q", len(frames)) + b"".join(frames)
    secs = [(1, state() if state_bytes is None else state_bytes), (2, table), (3, b"family"), (4, b"program")]
    if extra_tag is not None:
        secs.append((extra_tag, b"values"))
    out = b"RRPBIN\r\n" + struct.pack("<I", 1) + bytes([1, 0, 0, 0]) + struct.pack("<I", len(secs))
    for tag, payload in secs:
        out += struct.pack("<HHQ", tag, 0, len(payload)) + payload
    return out


class ConvertNativeV5ToV6Test(unittest.TestCase):
    def test_rewrites_only_the_state_version_and_frame_format_bytes(self):
        source = program()
        out = CONV.rewrite(source)
        self.assertEqual(len(out), len(source))
        diff = [i for i, (a, b) in enumerate(zip(source, out)) if a != b]
        state_offset = 20 + 12
        self.assertEqual(diff[0], state_offset + 4)
        self.assertEqual(struct.unpack_from("<H", out, state_offset + 4)[0], 6)
        self.assertEqual(len(diff), 3)  # version low byte + two frame format bytes
        for index in diff[1:]:
            self.assertEqual((source[index], out[index]), (0, 1))
        with self.assertRaisesRegex(CONV.Refused, "already Symbolica export format 6"):
            CONV.rewrite(out)

    def test_refuses_inputs_it_cannot_prove_header_only(self):
        cases = {
            "format 4": program(state_bytes=state(version=4)),
            "not a Num atom": program(frames=[frame(atom_type=3)]),
            "atom format 1": program(frames=[frame(fmt=1)]),
            "section tag 7": program(extra_tag=7),
            "user data tag 3": program(state_bytes=state(user_data=b"\x03")),
            "embedded atom": program(state_bytes=state(variable_kind=2)),
            "magic": b"NOTRRBIN" + bytes(20),
            "truncated": program()[:-3],
        }
        for message, data in cases.items():
            with self.subTest(message), self.assertRaisesRegex(CONV.Refused, message):
                CONV.rewrite(data)

    def test_tree_conversion_writes_a_new_directory_with_a_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp) / "src", Path(tmp) / "dst"
            (src / "owners").mkdir(parents=True)
            (src / "owners" / "a.rrbin").write_bytes(program())
            (src / "selection.json").write_text("{}\n")
            manifest = CONV.convert_tree(src, dst)
            self.assertEqual((manifest["rewritten"], manifest["copied"]), (1, 1))
            self.assertEqual((dst / "selection.json").read_text(), "{}\n")
            self.assertEqual((dst / "owners" / "a.rrbin").read_bytes(), CONV.rewrite(program()))
            on_disk = json.loads((dst / CONV.MANIFEST).read_text())
            self.assertEqual(on_disk["files"], manifest["files"])
            with self.assertRaises(CONV.Refused):
                CONV.convert_tree(src, dst)  # destination exists
            (src / "owners" / "b.rrbin").write_bytes(CONV.rewrite(program()))
            again = Path(tmp) / "again"
            self.assertEqual(CONV.main([str(src), str(again)]), 1)
            self.assertFalse(again.exists())


if __name__ == "__main__":
    unittest.main()
