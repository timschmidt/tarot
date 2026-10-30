#!/usr/bin/env python3
"""Read-only Linux IIO/LED capability inventory for the double-chop investigation.

Never enables motion events, changes wake sources, uploads firmware, samples IIO
buffers, changes LED triggers or lights a flash. Existence is not qualification.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path


def read(path):
    try:
        return path.read_text().strip()
    except FileNotFoundError:
        return None
    except OSError as error:
        return {'error':type(error).__name__}


def inventory():
    result={'read_only':True,'inspection_uid':os.geteuid(),'kernel':os.uname().release,
            'probe_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'iio':[],'leds':[],'modules':{},'config':[]}
    for device in sorted(Path('/sys/bus/iio/devices').glob('iio:device*')):
        real=device.resolve();parent=real.parent
        item={'name':read(device/'name'),'class_path':str(device),'device_path':str(real),
              'driver':str((parent/'driver').resolve()),'attributes':sorted(p.name for p in device.iterdir()),
              'events':{},'power':{},'device_tree':{}}
        for key in ['sampling_frequency','in_accel_scale','in_accel_mount_matrix','in_anglvel_scale']:
            value=read(device/key)
            if value is not None:item[key]=value
        for path in sorted((device/'events').glob('*')):
            if path.is_file():item['events'][path.name]=read(path)
        item['dmp_named_attributes']=[n for n in item['attributes'] if 'dmp' in n.lower() or 'firmware' in n.lower()]
        for ancestor in [parent,*list(parent.parents)[:3]]:
            values={}
            for key in ['power/wakeup','power/control','power/runtime_status']:
                value=read(ancestor/key)
                if value is not None:values[key]=value
            if values:item['power'][str(ancestor)]=values
        for key in ['compatible','interrupt-parent','interrupts','wakeup-source']:
            path=parent/'of_node'/key
            try:item['device_tree'][key]=path.read_bytes().hex()
            except FileNotFoundError:pass
            except OSError as error:item['device_tree'][key]={'error':type(error).__name__}
        result['iio'].append(item)
    for device in sorted(Path('/sys/class/leds').glob('*')):
        item={'name':device.name,'device_path':str(device.resolve()),
              'driver':str((device/'device/driver').resolve())}
        for key in ['brightness','max_brightness','trigger','flash_brightness','flash_max_brightness','flash_timeout','flash_max_timeout','flash_strobe','flash_fault']:
            value=read(device/key)
            if value is not None:item[key]=value
        result['leds'].append(item)
    for name in ['inv_mpu6050','inv_mpu6050_i2c','leds_sgm3140']:
        directory=Path('/sys/module')/name
        if directory.exists():result['modules'][name]={key:read(directory/key) for key in ['version','srcversion','taint']}
    config=Path('/proc/config.gz')
    try:
        result['config']=[line for line in gzip.decompress(config.read_bytes()).decode().splitlines()
                          if any(key in line for key in ['INV_MPU6050','LEDS_SGM3140','LEDS_CLASS_FLASH','IIO_TRIGGERED_BUFFER'])]
    except FileNotFoundError:pass
    except OSError as error:result['config_error']=type(error).__name__
    result['motion_interrupts']=[line for line in (read(Path('/proc/interrupts')) or '').splitlines() if 'inv_mpu' in line]
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path)
    args=parser.parse_args();payload=json.dumps(inventory(),indent=2)+'\n'
    if args.output:
        with args.output.open('x') as stream:stream.write(payload)
    else:print(payload,end='')


if __name__=='__main__':main()
