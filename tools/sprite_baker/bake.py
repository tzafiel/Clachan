import bpy
import sys
import os
import math
import subprocess
import tempfile
from mathutils import Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opts = {
        "input": None,
        "output": None,
        "palette": os.path.join(ROOT, "art_source", "clachan_master_64.gpl"),
        "width": 64,
        "height": 64,
        "angles": 4,
        "pitch": 42.0,
        "dither": 10.0,
    }
    i = 0
    while i < len(argv):
        k = argv[i].lstrip("-")
        if k in opts and i + 1 < len(argv):
            val = argv[i + 1]
            opts[k] = int(val) if k in ("width", "height", "angles") else (
                float(val) if k in ("pitch", "dither") else val
            )
            i += 2
        else:
            i += 1
    if not opts["input"] or not opts["output"]:
        raise SystemExit(
            "Usage: blender -b -P bake.py -- --input m.glb --output out.png "
            "--width 64 --height 64 --angles 4"
        )
    return opts


def clear_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def import_and_normalize(filepath):
    ext = os.path.splitext(filepath)[1].lower()
    if ext in (".glb", ".gltf"):
        bpy.ops.import_scene.gltf(filepath=filepath)
    elif ext == ".fbx":
        bpy.ops.import_scene.fbx(filepath=filepath)
    elif ext == ".obj":
        bpy.ops.wm.obj_import(filepath=filepath)
    else:
        raise SystemExit(f"Unsupported 3D format: {ext}")

    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not meshes:
        raise SystemExit("No mesh objects found in model.")

    # Compute combined world bounding box
    mins = Vector((1e9, 1e9, 1e9))
    maxs = Vector((-1e9, -1e9, -1e9))
    for obj in meshes:
        for corner in obj.bound_box:
            v = obj.matrix_world @ Vector(corner)
            mins = Vector((min(mins.x, v.x), min(mins.y, v.y), min(mins.z, v.z)))
            maxs = Vector((max(maxs.x, v.x), max(maxs.y, v.y), max(maxs.z, v.z)))

    center = (mins + maxs) * 0.5
    size = maxs - mins
    scale = 2.0 / max(size.x, size.y, size.z, 1e-4)

    pivot = bpy.data.objects.new("ModelPivot", None)
    bpy.context.scene.collection.objects.link(pivot)
    for obj in list(bpy.context.scene.objects):
        if obj != pivot and obj.parent is None:
            obj.parent = pivot
            obj.location -= Vector((center.x, center.y, mins.z))
    pivot.scale = (scale, scale, scale)
    return pivot


def setup_rig(width, height, pitch_deg):
    scene = bpy.context.scene
    try:
        scene.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        scene.render.engine = "BLENDER_EEVEE"

    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.filter_size = 0.01  # Zero anti-aliasing blur ("anti-mixel")
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.view_settings.view_transform = "Standard"

    # Cool indigo-teal shadow fill so undersides stay rich, never muddy black
    world = bpy.data.worlds.new("ClachanWorld")
    scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.14, 0.18, 0.28, 1.0)
        bg.inputs[1].default_value = 0.7

    # Upper-left (NW) warm directional sunlight
    sun_data = bpy.data.lights.new("ClachanSun", type="SUN")
    sun_data.energy = 3.2
    sun_data.color = (1.0, 0.96, 0.86)
    sun = bpy.data.objects.new("ClachanSun", sun_data)
    scene.collection.objects.link(sun)
    sun.rotation_euler = (math.radians(48), math.radians(-28), math.radians(-35))

    # Orthographic 3/4 top-down camera aimed at model center (Z = 1.0)
    cam_data = bpy.data.cameras.new("OrthoCam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 2.65  # Leaves ~2px breathing room for 1px outline
    cam = bpy.data.objects.new("OrthoCam", cam_data)
    scene.collection.objects.link(cam)
    pitch = math.radians(pitch_deg)
    dist = 10.0
    cam.location = (0.0, -dist * math.cos(pitch), 1.0 + dist * math.sin(pitch))
    cam.rotation_euler = (math.radians(90.0 - pitch_deg), 0.0, 0.0)
    scene.camera = cam


def main():
    opts = parse_args()
    clear_scene()
    pivot = import_and_normalize(opts["input"])
    setup_rig(opts["width"], opts["height"], opts["pitch"])

    with tempfile.TemporaryDirectory() as tmp:
        raw_frames = []
        for i in range(opts["angles"]):
            pivot.rotation_euler = (0.0, 0.0, math.radians(i * (360.0 / opts["angles"])))
            frame_path = os.path.join(tmp, f"frame_{i:02d}.png")
            bpy.context.scene.render.filepath = frame_path
            bpy.ops.render.render(write_still=True)
            raw_frames.append(frame_path)

        # Pipe raw frames straight into Rust clachan_forge to snap palette + add 1px outline
        cmd = [
            "cargo",
            "run",
            "--release",
            "--manifest-path",
            os.path.join(ROOT, "tools", "clachan_forge", "Cargo.toml"),
            "--",
            "flash",
            opts["palette"],
            opts["output"],
            "--dither",
            str(opts["dither"]),
            *raw_frames,
        ]
        subprocess.run(cmd, check=True)


if __name__ == "__main__":
    main()
