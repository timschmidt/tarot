"""A capture made stale by keyboard remapping must never supply a tap target."""
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest
from PIL import Image, ImageDraw
from browser_target import painted_target, settled


def scene(keyboard=False,fullscreen=False):
    height=(444 if keyboard else 720) if fullscreen else (412 if keyboard else 652)
    rect=dict(x=0.0,y=0.0 if fullscreen else 32.0,width=360.0,height=float(height))
    return {'view':{'App':1},'keyboard_visible':keyboard,
            'keyboard_rect':dict(x=0.0,y=444.0 if keyboard else 684.0,width=360.0,height=240.0 if keyboard else 0.0),
            'app_area':dict(x=0.0,y=32.0,width=360.0,height=412.0 if keyboard else 652.0),
            'native_geometry':[dict(id=1,fullscreen=fullscreen,geometry=dict(x=0,y=0,width=360,height=height))],
            'presentation':dict(overview=0,dock=0,windows=[dict(id=1,rect=rect,opacity=1,rotation=0,corner_radius=0)]),
            'output':dict(orientation='normal',requested=None,revision=1,presented_revision=1)}


class BrowserTargetTests(unittest.TestCase):
    def acquire(self,before,after):
        self.captures=[];self.rejected=[]
        candidates=iter(before);observations=iter(after)
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)
            def state(label,predicate):
                while True:
                    value=next(candidates)
                    if predicate(value):return value
            def capture(label):
                self.captures.append(label)
                image=Image.new('RGB',(720,1440))
                ImageDraw.Draw(image).rectangle((50,200,669,279),fill=(197,223,249))
                image.save(out/(label+'.png'))
            return painted_target('primary','refocus',state,capture,lambda:next(observations),
                                  out,(197,223,249),self.rejected)

    def test_recaptures_when_keyboard_maps_during_pixel_analysis(self):
        result=self.acquire([scene(),scene(True)],[scene(True),scene(True)])
        self.assertEqual(self.captures,['refocus-0','refocus-1'])
        self.assertEqual(result['capture'],'refocus-1')
        self.assertEqual(len(self.rejected),1)
        self.assertEqual(result['point'],(180,120))

    def test_waits_for_native_commit_and_rejects_repeated_invalidations(self):
        pending=scene(True);pending['native_geometry'][0]['geometry']['height']=652
        self.assertFalse(settled(pending))
        result=self.acquire([pending,scene(True)],[scene(True)])
        self.assertEqual(self.captures,['refocus-0'])
        self.assertEqual(result['point'],(180,120))
        with self.assertRaisesRegex(AssertionError,'target_not_stable_or_visible'):
            self.acquire([scene() for _ in range(5)],[scene(True) for _ in range(5)])
        self.assertEqual(len(self.rejected),5)

    def test_fullscreen_uses_its_reserved_size_and_orientation_must_be_presented(self):
        for keyboard in [False,True]:self.assertTrue(settled(scene(keyboard,True)))
        moving=scene(True);moving['presentation']['windows'][0]['rect']['height']=463.14
        self.assertFalse(settled(moving))
        rotated=deepcopy(scene());rotated['output']['revision']=2
        self.assertFalse(settled(rotated))
        rotated['output']['presented_revision']=2;rotated['output']['orientation']='left-up'
        self.assertFalse(settled(rotated))
        self.assertFalse(settled({'view':'Overview'}))


if __name__=='__main__':unittest.main()
