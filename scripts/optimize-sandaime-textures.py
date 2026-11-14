"""Lossless, metadata-preserving PNG recompression inside the authored GLB.

Offline asset maintenance only; runtime/builds do not depend on Pillow.
Usage: python scripts/optimize-sandaime-textures.py ORIGINAL.glb OUTPUT.glb REPORT.json
The original and output paths must differ. Geometry/rig/animation bytes stay exact.
"""
from pathlib import Path
import copy
import hashlib
import io
import json
import struct
import sys
import zlib

SIGNATURE = b"\x89PNG\r\n\x1a\n"


def chunks(data):
    assert data.startswith(SIGNATURE), "Expected embedded PNG"
    offset = 8
    result = []
    while offset < len(data):
        size, kind = struct.unpack_from(">I4s", data, offset)
        end = offset + size + 12
        assert end <= len(data), "Truncated PNG chunk"
        payload = data[offset + 8:offset + 8 + size]
        checksum = struct.unpack_from(">I", data, offset + 8 + size)[0]
        assert zlib.crc32(kind + payload) & 0xffffffff == checksum, "PNG checksum mismatch"
        result.append((kind, payload, data[offset:end]))
        offset = end
    assert result[-1][0] == b"IEND"
    return result


def compress_png(original):
    from PIL import Image
    old = chunks(original)
    image = Image.open(io.BytesIO(original))
    # Fixed source assets use RGBA8; unsupported inputs are left to their authoring tools.
    assert image.mode == "RGBA" and not image.info.get("interlace"), "Unsupported PNG layout"
    encoded = io.BytesIO()
    image.save(encoded, format="PNG", optimize=True, compress_level=9)
    new = chunks(encoded.getvalue())
    assert old[0][:2] == new[0][:2], "PNG pixel layout changed"
    replacement = b"".join(raw for kind, _, raw in new if kind == b"IDAT")
    result = bytearray(SIGNATURE)
    inserted = False
    for kind, _, raw in old:
        if kind == b"IDAT":
            if not inserted:
                result.extend(replacement)
                inserted = True
        else:
            # Preserve color profiles/gamma and every other original ancillary chunk.
            result.extend(raw)
    result = bytes(result)
    decoded = Image.open(io.BytesIO(result))
    assert image.size == decoded.size and image.tobytes() == decoded.tobytes(), "Pixel content changed"
    assert [item for item in old if item[0] != b"IDAT"] == [item for item in chunks(result) if item[0] != b"IDAT"]
    return result if len(result) < len(original) else original


def optimize(source, destination, report):
    from PIL import Image, __version__ as pillow_version
    assert source.resolve() != destination.resolve(), "Preserve the original GLB"
    raw = source.read_bytes()
    magic, version, length, json_size, json_type = struct.unpack_from("<5I", raw)
    assert (magic, version, length, json_type) == (0x46546c67, 2, len(raw), 0x4e4f534a)
    binary_size, binary_type = struct.unpack_from("<2I", raw, 20 + json_size)
    assert binary_type == 0x004e4942 and 28 + json_size + binary_size == len(raw)
    doc = json.loads(raw[20:20 + json_size])
    original_doc = copy.deepcopy(doc)
    assert len(doc["buffers"]) == 1 and "uri" not in doc["buffers"][0]
    start = 28 + json_size
    image_views = {image["bufferView"] for image in doc["images"] if image["mimeType"] == "image/png"}
    assert len(image_views) == len(doc["images"]) == 10
    output = bytearray()
    images = []
    preserved = []
    sha = lambda data: hashlib.sha256(data).hexdigest()
    for index, view in enumerate(doc["bufferViews"]):
        assert view["buffer"] == 0
        offset = start + view.get("byteOffset", 0)
        original = raw[offset:offset + view["byteLength"]]
        assert len(original) == view["byteLength"]
        transformed = compress_png(original) if index in image_views else original
        if index in image_views:
            image = Image.open(io.BytesIO(original))
            images.append({"bufferView": index, "beforeBytes": len(original), "afterBytes": len(transformed),
                           "width": image.width, "height": image.height, "rgbaSha256": sha(image.tobytes()),
                           "pixelsIdentical": True, "metadataIdentical": True})
        else:
            preserved.append({"bufferView": index, "bytes": len(original), "sha256": sha(original)})
        output.extend(b"\0" * (-len(output) % 4))
        view["byteOffset"] = len(output)
        view["byteLength"] = len(transformed)
        output.extend(transformed)
    doc["buffers"][0]["byteLength"] = len(output)
    output.extend(b"\0" * (-len(output) % 4))
    encoded_doc = json.dumps(doc, ensure_ascii=False, separators=(",", ":")).encode()
    encoded_doc += b" " * (-len(encoded_doc) % 4)
    result = struct.pack("<5I", magic, 2, 28 + len(encoded_doc) + len(output), len(encoded_doc), json_type)
    result += encoded_doc + struct.pack("<2I", len(output), binary_type) + output
    normalized = copy.deepcopy(doc)
    normalized["buffers"] = original_doc["buffers"]
    normalized["bufferViews"] = original_doc["bufferViews"]
    assert normalized == original_doc, "Scene/material/animation metadata changed"
    for item in preserved:
        view = doc["bufferViews"][item["bufferView"]]
        assert sha(output[view["byteOffset"]:view["byteOffset"] + view["byteLength"]]) == item["sha256"]
    assert len(result) < len(raw), "No improvement"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(result)
    receipt = {"method": "lossless PNG IDAT replacement; original IHDR and ancillary chunks retained", "pillow": pillow_version,
               "beforeSha256": sha(raw), "afterSha256": sha(result), "beforeBytes": len(raw), "afterBytes": len(result),
               "images": images, "nonImageViewsPreserved": preserved, "sceneMaterialsAnimationsIdentical": True,
               "gpuTextureDimensionsUnchanged": True}
    report.write_text(json.dumps(receipt, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"beforeBytes": len(raw), "afterBytes": len(result), "savedBytes": len(raw) - len(result), "pixelsIdentical": True}))


if __name__ == "__main__":
    assert len(sys.argv) == 4, __doc__
    optimize(*(Path(value) for value in sys.argv[1:]))
