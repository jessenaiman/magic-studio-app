#!/usr/bin/env python3
"""Build assets/catalog.json: one entry per usable asset (3D characters, sets, images, looks).

usage: build_catalog.py            # fast file scan, keeps probed data from the previous catalog
       build_catalog.py --probe    # also runs Blender on 3D assets missing animations/thumbnails
"""
import json, os, re, subprocess, sys
from pathlib import Path

ROOT = Path('/run/media/jesse/Speedy/magic')
A = ROOT / 'assets'
OUT = A / 'catalog.json'
PROBE = ROOT / 'tools' / 'catalog_probe.py'
MODEL_EXT = ('.glb', '.gltf', '.fbx')

KID_UNSAFE = {'monsters'}
LICENSES = {
    'free': ('CC0-1.0', 'Quaternius (quaternius.com)'),
    'tripo': ('own', 'Tripo Studio generation from Jesse\'s character art'),
    'image': ('own', 'GPT Image 2.5 from Jesse\'s character references'),
}


def slug(s):
    return re.sub(r'[^a-z0-9]+', '-', s.lower()).strip('-')


def shelf_for(rel: Path):
    p = rel.parts
    if p[0] == 'characters':
        if p[1] == 'free':
            return p[2], 'free'                      # sea / animals / dinosaurs / monsters
        if p[1] in ('farm-school', 'moppets', 'nelly', 'zoo', 'barnyard-band', 'base-bodies', 'families', 'hey-ho-fish', 'baby-shark'):
            return p[1], 'tripo'
        if p[1] == 'reskins':
            return 'reskins', 'tripo'
    if p[0] in ('sets',):
        return 'sets/' + p[1], 'free'
    if p[0] == 'backgrounds':
        return 'backgrounds', 'image'
    if p[0] == 'library':
        return 'looks', 'own'
    return p[0], 'own'


def entry(path: Path, kind: str):
    rel = path.relative_to(A)
    shelf, src = shelf_for(rel)
    name = path.parent.name if path.stem.startswith('tripo_convert') else path.stem
    lic, source = LICENSES.get(src, ('own', 'Jesse / Magic Studio'))
    tags = [shelf.split('/')[0]]
    if shelf not in KID_UNSAFE: tags.append('kid-safe')
    return {
        'id': f"{shelf}/{slug(name)}" + ('' if kind == 'model' else f':{kind}'),
        'name': name.replace('-', ' ').title(), 'kind': kind, 'shelf': shelf,
        'path': str(rel), 'format': path.suffix.lstrip('.').lower(),
        'license': lic, 'source': source, 'tags': tags,
        'rig': None, 'animations': [], 'thumbnail': None,
    }


def scan():
    items = {}
    chars = A / 'characters'
    # 3D models: one per character folder, prefer glb > fbx; skip _incoming
    for f in sorted(chars.rglob('*')):
        if f.suffix.lower() not in MODEL_EXT or '_incoming' in f.parts or '.fbm' in str(f): continue
        e = entry(f, 'model'); key = e['id']
        if key in items and items[key]['format'] == 'glb': continue
        items[key] = e
    # sets (3D kits) - one entry per model file
    for f in sorted((A / 'sets').rglob('*')) if (A / 'sets').exists() else []:
        if f.suffix.lower() in ('.glb', '.gltf') or (f.suffix.lower() == '.fbx' and not f.with_suffix('.gltf').exists()):
            e = entry(f, 'model'); items.setdefault(e['id'], e)
    # 2D reference images: T-poses, turnarounds, sheets, backgrounds
    for sub, kind in (('tpose', 'tpose'), ('turnarounds', 'turnaround'), ('props', 'props'), ('costumes', 'costume'), ('sheets', 'sheet')):
        for f in sorted(chars.rglob(f'{sub}/*.png')):
            e = entry(f, kind); items.setdefault(e['id'], e)
    for f in sorted((A / 'backgrounds').rglob('*.png')) if (A / 'backgrounds').exists() else []:
        e = entry(f, 'background'); items.setdefault(e['id'], e)
    for f in sorted((A / 'library').glob('*.blend')):
        e = entry(f, 'look'); items.setdefault(e['id'], e)
    return items


def main():
    old = {}
    if OUT.exists():
        old = {e['id']: e for e in json.loads(OUT.read_text()).get('assets', [])}
    items = scan()
    for k, e in items.items():          # keep probed fields
        if k in old:
            for fld in ('rig', 'animations', 'thumbnail', 'tags'):
                if old[k].get(fld): e[fld] = old[k][fld]
    if '--probe' in sys.argv:
        todo = [e for e in items.values() if e['kind'] == 'model' and e['thumbnail'] is None and e['shelf'].split('/')[0] != 'sets']
        for i, e in enumerate(todo, 1):
            thumb = A / 'thumbnails' / (e['id'].replace('/', '__') + '.png')
            thumb.parent.mkdir(parents=True, exist_ok=True)
            r = subprocess.run(['blender-5.3', '-b', '--python', str(PROBE), '--', str(A / e['path']), str(thumb)],
                               capture_output=True, text=True, timeout=600)
            m = re.search(r'PROBE (\{.*\})', r.stdout)
            if m:
                info = json.loads(m.group(1)); e['rig'] = info['rig']; e['animations'] = info['animations']
                if thumb.exists(): e['thumbnail'] = str(thumb.relative_to(A))
            print(f"[{i}/{len(todo)}] {e['id']}: rig={e['rig']} anims={len(e['animations'])}", flush=True)
            OUT.write_text(json.dumps({'version': 1, 'root': str(A), 'assets': sorted(items.values(), key=lambda x: x['id'])}, indent=1))
    OUT.write_text(json.dumps({'version': 1, 'root': str(A), 'assets': sorted(items.values(), key=lambda x: x['id'])}, indent=1))
    by = {}
    for e in items.values(): by.setdefault(f"{e['shelf']} ({e['kind']})", 0); by[f"{e['shelf']} ({e['kind']})"] += 1
    print(f"{len(items)} assets -> {OUT}"); [print(f"  {k}: {v}") for k, v in sorted(by.items())]


if __name__ == '__main__':
    main()
