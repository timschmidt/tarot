#!/usr/bin/env python3
"""Export hash-verified QA captures as a small animation without interpolation."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('directory',type=Path)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--package-audit',type=Path)
    p.add_argument('--width',type=int,default=360)
    args=p.parse_args();assert 64<=args.width<=1440
    trace=args.directory/'frames.json';record=json.loads(trace.read_text());assert not record['error'] and not record['interpolated']
    frames=record['frames'];assert len(frames)>=2
    images=[]
    for f in frames:
        assert Path(f['file']).name==f['file']
        file=args.directory/f['file'];assert hashlib.sha256(file.read_bytes()).hexdigest()==f['sha256']
        with Image.open(file) as source:source.load();im=source.convert('RGB')
        im.thumbnail((args.width,args.width*4),Image.Resampling.LANCZOS);images.append(im)
    assert all(im.size==images[0].size for im in images)
    palette=Image.new('RGB',(images[0].width,images[0].height*3))
    for row,index in enumerate([0,len(images)//2,len(images)-1]):palette.paste(images[index],(0,row*images[0].height))
    palette=palette.quantize(colors=256)
    quantized=[im.quantize(palette=palette,dither=Image.Dither.NONE) for im in images]
    durations=[]
    for a,b in zip(frames,frames[1:]):
        delta=b['completed_seconds']-a['completed_seconds'];assert delta>0
        durations.append(max(10,round(delta*100)*10))
    durations.append(400)  # Explicit endpoint display hold, not a captured interval.
    args.output.parent.mkdir(parents=True,exist_ok=True)
    quantized[0].save(args.output,save_all=True,append_images=quantized[1:],duration=durations,loop=0,optimize=False,disposal=2)
    with Image.open(args.output) as encoded:encoded_count=encoded.n_frames
    meta={'encoded_frame_count':encoded_count,'identical_quantized_frames_may_coalesce':True,'source_trace_sha256':hashlib.sha256(trace.read_bytes()).hexdigest(),'actual_capture_count':len(frames),
        'source_capture_seconds':frames[-1]['completed_seconds']-frames[0]['completed_seconds'],
        'gif_sha256':hashlib.sha256(args.output.read_bytes()).hexdigest(),'output_size':images[0].size,
        'interpolated':False,'capture_intervals_quantized_to_10ms':True,'endpoint_display_hold_ms':400,
        'colors_quantized':True,'performance_benchmark':False,'frames':frames}
    if args.package_audit:
        audit=json.loads(args.package_audit.read_text());assert audit['validation']['passed']
        meta['source_archive_sha256']=audit['source_manifest']['source_sha256'];meta['package_sha256']=audit['package_sha256'];meta['binaries_sha256']=audit['binaries_sha256']
    args.output.with_suffix('.json').write_text(json.dumps(meta,indent=2)+'\n')
    print(args.output,':',len(frames),'actual captures,',args.output.stat().st_size,'bytes')


if __name__=='__main__':main()
