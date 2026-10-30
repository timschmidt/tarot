"""Expected synthetic broadcast metadata, independent of the Rust emitter."""
SIZE=96*1024
APP='/org/tarot/MenuEgui/app';WINDOW='/org/tarot/MenuEgui/window';MENU='/org/tarot/MenuEgui/menu';OTHER='/org/tarot/Noise'
UNRELATED=[
 ('unrelated','fixture',OTHER,'org.tarot.Noise','Changed'),
 ('actions-path','fixture',OTHER,'org.gtk.Actions','Changed'),
 ('actions-interface','fixture',APP,'org.tarot.Noise','Changed'),
 ('actions-member','fixture',APP,'org.gtk.Actions','Noise'),
 ('menus-path','fixture',OTHER,'org.gtk.Menus','Changed'),
 ('menus-interface','fixture',MENU,'org.tarot.Noise','Changed'),
 ('menus-member','fixture',MENU,'org.gtk.Menus','Noise'),
 ('app-relevant','foreign',APP,'org.gtk.Actions','Changed'),
]
RELEVANT=[('app-relevant','fixture',APP,'org.gtk.Actions','Changed'),('window-relevant','fixture',WINDOW,'org.gtk.Actions','Changed'),('menu-relevant','fixture',MENU,'org.gtk.Menus','Changed')]
COUNT=len(UNRELATED)+len(RELEVANT)
def verify(record,case,owners):
 _,name,path,interface,member=case
 assert {k:record[k] for k in ['sender','path','interface','member','payload_bytes']}=={'sender':owners[name],'path':path,'interface':interface,'member':member,'payload_bytes':SIZE},record
