#!/usr/bin/env python3
"""Existing GLB -> reusable rig -> Mixamo -> measured checks -> review candidate.
No generation service, no credits, no source overwrite, no automatic visual approval.
"""
import argparse, hashlib, json, os, shutil, struct, subprocess, sys, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def glb_info(path):
    with path.open('rb') as f:
        h=f.read(20)
        if len(h)!=20 or h[:4]!=b'glTF' or h[16:20]!=b'JSON':raise ValueError('Input must be a GLB')
        doc=json.loads(f.read(struct.unpack('<I',h[12:16])[0]))
    return {'skins':len(doc.get('skins',[])),'joints':max((len(s.get('joints',[])) for s in doc.get('skins',[])),default=0),'clips':[a.get('name') for a in doc.get('animations',[])],'vertices':sum(doc['accessors'][p['attributes']['POSITION']]['count'] for m in doc.get('meshes',[]) for p in m['primitives'])}
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source',required=True,type=Path);p.add_argument('--character',required=True)
    p.add_argument('--out-dir',required=True,type=Path);p.add_argument('--blender',default='blender-5.3')
    p.add_argument('--profile',type=Path,default=ROOT/'tools/rig-profiles/stylized-animal-biped.json')
    p.add_argument('--template-rig',type=Path);p.add_argument('--rig-only',action='store_true');p.add_argument('--inspect',action='store_true')
    p.add_argument('--demo-manifest',type=Path,help='Register candidate in an existing demo manifest; outputs must be beneath its folder')
    a=p.parse_args();a.source=a.source.resolve();a.out_dir=a.out_dir.resolve()
    if not a.character or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-_' for c in a.character):p.error('Use a lowercase character ID')
    info=glb_info(a.source)
    if a.inspect:print(json.dumps(info,indent=2));return
    a.out_dir.mkdir(parents=True,exist_ok=True)
    report={'source':str(a.source),'source_sha256':hashlib.sha256(a.source.read_bytes()).hexdigest(),'source_info':info,'character':a.character,'status':'running','stages':[],'started_unix':time.time()}
    report_path=a.out_dir/f'{a.character}-pipeline.json'
    def write():
        pending=report_path.with_suffix('.json.tmp');pending.write_text(json.dumps(report,indent=2)+'\n');os.replace(pending,report_path)
    def run(stage,script,args):
        log=a.out_dir/f'{a.character}-{stage}.log';row={'stage':stage,'log':str(log),'status':'running'};report['stages'].append(row);write()
        cmd=[a.blender,'-b','--factory-startup','--python-exit-code','1','--python',str(script),'--',*map(str,args)]
        print(stage,flush=True)
        with log.open('w') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
        row['exit_code']=r.returncode;row['status']='passed' if r.returncode==0 else 'failed';write()
        if r.returncode:raise RuntimeError(f'{stage} failed; inspect {log}')
    try:
        candidate=a.source
        if not info['joints']:
            if not a.template_rig:p.error('An unrigged mesh needs --template-rig (a compatible Mixamo skeleton)')
            candidate=a.out_dir/f'{a.character}-rig-candidate.glb'
            if candidate==a.source:raise ValueError('Source and candidate paths must differ')
            run('bind',ROOT/'tools/rig_existing_mesh.py',['--source',a.source,'--out',candidate,'--template-rig',a.template_rig.resolve(),'--profile',a.profile.resolve()])
            bound=glb_info(candidate)
            if bound['vertices']!=info['vertices']:raise RuntimeError('Rigging changed the vertex count')
            if bound['joints']!=65:raise RuntimeError('Expected the Mixamo-compatible 65-joint skeleton')
        report['rig']=str(candidate)
        if not a.rig_only:
            output=a.out_dir/f'{a.character}-moves.glb'
            run('animate',ROOT/'tools/add_mixamo_moves.py',[candidate,output])
            report['animation']=str(output);report['output_info']=glb_info(output)
            report['technical_checks']='retarget fidelity, proxy clearance, bone lengths, full-frame mesh tear checks; see sidecars'
            if a.demo_manifest:
                manifest=a.demo_manifest.resolve();relative=output.relative_to(manifest.parent);data=json.loads(manifest.read_text())
                entry={'id':a.character,'name':a.character.replace('-',' ').title(),'url':relative.as_posix(),'version':str(output.stat().st_mtime_ns),'allClips':report['output_info']['clips'],'joints':report['output_info']['joints'],'reviewed':False,'qaStatus':'needs-visual-review'}
                data['models']=[m for m in data.get('models',[]) if m['id']!=a.character]+[entry]
                pending=manifest.with_suffix('.json.tmp');pending.write_text(json.dumps(data,indent=2)+'\n');os.replace(pending,manifest)
        report['status']='needs-visual-review';write();print(json.dumps({'report':str(report_path),'status':report['status'],'animation':report.get('animation')}))
    except Exception as e:
        report['status']='failed';report['error']=str(e);write();raise
if __name__=='__main__':main()
