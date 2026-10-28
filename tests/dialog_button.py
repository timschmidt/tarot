"""Locate a rendered dialog button, excluding its similarly named title.

This is a QA helper for synthetic editor captures, not a runtime OCR dependency.
"""
import csv
import io
from pathlib import Path
import subprocess
from PIL import Image,ImageOps


def locate(image_path,word,program='tesseract',vertical=(.45,.9)):
    image_path=Path(image_path)
    with Image.open(image_path) as source:source.load();image=source.convert('RGB')
    # The tested narrow GTK layouts put dialog responses below the document
    # list. Never let a title such as "Save Changes?" satisfy a Save response.
    assert 0<=vertical[0]<vertical[1]<=1
    top=round(image.height*vertical[0]);bottom=round(image.height*vertical[1])
    crop=image.crop((0,top,image.width,bottom));attempts=[]
    for variant,picture in [('crop',crop),('contrast',ImageOps.grayscale(crop).point(lambda value:0 if value>200 else 255))]:
        path=image_path.with_name(image_path.stem+'-buttons-'+variant+'.png');picture.save(path)
        for mode in [3,6,11]:
            result=subprocess.run([program,str(path),'stdout','--psm',str(mode),'tsv'],capture_output=True,text=True,check=True,timeout=30)
            rows=[r for r in csv.DictReader(io.StringIO(result.stdout),delimiter='\t') if r['text'].casefold()==word.casefold() and float(r['conf'])>=80]
            attempts.append({'variant':variant,'mode':mode,'matches':rows})
            if rows:
                row=max(rows,key=lambda r:int(r['top']));x=int(row['left'])+int(row['width'])/2;y=top+int(row['top'])+int(row['height'])/2
                return {'word':word,'point':(x,y),'recognized':row,'crop_top':top,'variant':variant,'mode':mode,'attempts':attempts}
    raise AssertionError({'word':word,'image':str(image_path),'attempts':attempts})
