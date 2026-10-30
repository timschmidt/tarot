"""Validate and package the complete Fx0 design deliverables locally."""
from pathlib import Path
import ast, json, shutil, subprocess, zipfile
import xml.etree.ElementTree as ET
from PIL import Image
from pypdf import PdfReader

ROOT=Path(__file__).resolve().parent.parent
DEST=ROOT.parent
PDF=ROOT/'Tarot_Fx0_Design_Packet.pdf'
SVG=list((ROOT/'screens/svg').glob('*.svg'))
PNG=list((ROOT/'screens/png').glob('*.png'))
assert len(SVG)==len(PNG)==26
for f in SVG:
    el=ET.parse(f).getroot()
    assert el.attrib['viewBox']=='0 0 360 640',f
    assert el.attrib['width']=='720' and el.attrib['height']=='1280',f
for f in PNG:
    with Image.open(f) as im: assert im.size==(720,1280),f
for f in ROOT.rglob('*.svg'): ET.parse(f)
for f in (ROOT/'source').glob('*.py'): ast.parse(f.read_text())
tokens=json.loads((ROOT/'tokens/tarot.tokens.json').read_text())
assert tokens['active_theme']=='fx0_gold'
assert (ROOT/'tokens/fx0-gold.theme.json').is_file()
assert (ROOT/'handoff/gold-contrast-checks.json').is_file()
assert len(list((ROOT/'wallpapers').glob('*.svg')))==4
assert len(list((ROOT/'wallpapers').glob('*.png')))==4
assert tokens['units']['output_px']==[720,1280]
assert tokens['geometry']['app_rect']==[0,32,360,572]
assert tokens['geometry']['card_anchor']==[61.2,112]
reader=PdfReader(PDF)
assert len(reader.pages)==33
assert 'v0.1' not in '\n'.join(p.extract_text() for p in reader.pages)
movie=ROOT/'motion/Tarot_Fx0_Card_Motion.mp4'
media=json.loads(subprocess.check_output(['ffprobe','-v','error','-select_streams','v:0','-show_entries','stream=width,height,r_frame_rate,duration','-of','json',str(movie)]))['streams'][0]
assert (media['width'],media['height'])==(720,1280)
summary={
 'edition':'Fx0 Gold 0.3',
 'date':'2026-10-30',
 'pdf_pages':33,
 'screen_svgs':26,
 'native_screen_pngs':26,
 'screen_output_px':[720,1280],
 'screen_viewbox':[0,0,360,640],
 'wallpaper_themes':4,
 'gold_screen_studies':23,
 'reference_theme_studies':3,
 'gold_contrast_report':'handoff/gold-contrast-checks.json',
 'app_icons':len(list((ROOT/'icons').glob('*.svg'))),
 'glyphs':len(list((ROOT/'glyphs').glob('*.svg'))),
 'tokens_valid_json':True,
 'all_svg_xml_valid':True,
 'movie':media,
 'visual_review':'All 26 screen studies and 33 PDF pages reviewed; Gold UI roles, control states and motion inspected.',
 'hardware_frame':'Original schematic, separate from screen exports.',
 'status':'Design proposal; device runtime, current port support and historical-capture conformance remain unverified.'
}
(ROOT/'handoff/validation-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
for src in [PDF,ROOT/'Tarot_Fx0_Visual_Overview.png',movie]:
    shutil.copyfile(src,DEST/src.name)
archive=DEST/'Tarot_Fx0_Design_Sources.zip'
with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as z:
    for f in sorted(ROOT.rglob('*')):
        rel=f.relative_to(ROOT)
        if f.is_file() and not any(part in ('tmp','__pycache__') for part in rel.parts):
            z.write(f,Path('tarot-fx0-gold-design-sources')/rel)
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
    summary['archive_entries']=len(z.infolist())
summary['archive_bytes']=archive.stat().st_size
print(json.dumps(summary,indent=2))
