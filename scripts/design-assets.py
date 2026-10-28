#!/usr/bin/env python3
"""Bake the owner's SVG artwork for the Rust PNG loader; never alter the packet.

Only development tooling uses CairoSVG/fontconfig. Runtime decode/render stays
Rust. Research fragments and complete screen mockups are never runtime assets.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
from xml.sax.saxutils import escape


def main():
    repo=Path(__file__).resolve().parent.parent
    source=repo/'docs/Tarot_Design_Packet'
    dest=repo/'crates/tarot-ui/assets'
    (dest/'fonts').mkdir(parents=True,exist_ok=True)
    (dest/'icons').mkdir(exist_ok=True)
    for name in ['Lato-Regular.ttf','Lato-Bold.ttf','Lato-Italic.ttf','InstrumentSerif-Regular.ttf','Lato-OFL.txt','InstrumentSerif-OFL.txt']:
        shutil.copyfile(source/'fonts'/name,dest/'fonts'/name)
    shutil.copyfile(source/'wallpapers/nocturne.png',dest/'nocturne.png')
    shutil.copyfile(source/'tokens/tarot.tokens.json',repo/'crates/tarot-core/assets/tarot.tokens.json')
    record={'source':'docs/Tarot_Design_Packet','originals_modified':False,'assets':{}}
    with tempfile.TemporaryDirectory(prefix='tarot-design-fonts-') as tmp:
        tmp=Path(tmp);config=tmp/'fonts.conf'
        config.write_text('<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "fonts.dtd"><fontconfig><dir>'+escape(str(source/'fonts'))+'</dir><cachedir>'+escape(str(tmp/'cache'))+'</cachedir></fontconfig>')
        os.environ['FONTCONFIG_FILE']=str(config)
        import cairosvg
        record['renderer']='CairoSVG '+cairosvg.__version__
        for path in sorted((source/'icons').glob('*.svg')):
            out=dest/'icons'/(path.stem+'.png')
            cairosvg.svg2png(bytestring=path.read_bytes(),write_to=str(out),output_width=128,output_height=128)
            record['assets'][str(out.relative_to(repo))]={'source':str(path.relative_to(repo)),'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'sha256':hashlib.sha256(out.read_bytes()).hexdigest()}
        # Preserve the complete icon study; a separate background lets Rust
        # render the actual local date instead of shipping the sample "28".
        original=source/'icons/calendar.svg'
        base,count=re.subn(r'<text\b[^>]*>28</text>', '', original.read_text())
        assert count==1,'Calendar study must have exactly one illustrative date'
        out=dest/'icons/calendar-base.png'
        cairosvg.svg2png(bytestring=base.encode(),write_to=str(out),output_width=128,output_height=128)
        record['assets'][str(out.relative_to(repo))]={'source':str(original.relative_to(repo)),
            'source_sha256':hashlib.sha256(original.read_bytes()).hexdigest(),
            'derivation':'Original artwork with illustrative date text removed; runtime supplies local day',
            'sha256':hashlib.sha256(out.read_bytes()).hexdigest()}
        # The resting tray is an original packet material, extracted as a
        # separate renderable surface rather than using any screenshot pixels.
        defs=re.search(r'<defs>.*?</defs>',(source/'icons/phone.svg').read_text(),re.S).group(0)
        tray='<svg xmlns="http://www.w3.org/2000/svg" width="708" height="138" viewBox="0 0 354 69">'+defs+'<rect x=".5" y=".5" width="353" height="68" rx="18" fill="url(#tray)" stroke="#C5CFC5" stroke-width=".8"/><path d="M20 1H334" stroke="#E5EADF" stroke-width=".7"/></svg>'
        (dest/'tray.svg').write_text(tray)
        cairosvg.svg2png(bytestring=tray.encode(),write_to=str(dest/'tray.png'),output_width=708,output_height=138)
    for source_path, out in [(source/'wallpapers/nocturne.png',dest/'nocturne.png'),
                             (source/'tokens/tarot.tokens.json',repo/'crates/tarot-core/assets/tarot.tokens.json')]:
        record['assets'][str(out.relative_to(repo))]={'source':str(source_path.relative_to(repo)),
            'source_sha256':hashlib.sha256(source_path.read_bytes()).hexdigest(),
            'sha256':hashlib.sha256(out.read_bytes()).hexdigest()}
    for out in sorted((dest/'fonts').iterdir()):
        source_path=source/'fonts'/out.name
        record['assets'][str(out.relative_to(repo))]={'source':str(source_path.relative_to(repo)),
            'source_sha256':hashlib.sha256(source_path.read_bytes()).hexdigest(),
            'sha256':hashlib.sha256(out.read_bytes()).hexdigest()}
    record['tray']={'material_source':'docs/Tarot_Design_Packet/icons/phone.svg#tray',
        'source_sha256':hashlib.sha256((source/'icons/phone.svg').read_bytes()).hexdigest(),
        'svg_sha256':hashlib.sha256((dest/'tray.svg').read_bytes()).hexdigest(),
        'png_sha256':hashlib.sha256((dest/'tray.png').read_bytes()).hexdigest()}
    (dest/'provenance.json').write_text(json.dumps(record,indent=2)+'\n')
    print('Baked original Nocturne artwork, tray, fonts and shared token snapshot')


if __name__=='__main__':main()
