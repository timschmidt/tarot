#!/usr/bin/env python3
"""External GTK3/Qt6 dialogs for compatibility QA; not a Tarot product app.

Only fixed synthetic fixture text is recorded. Actions come from the real
toolkit widgets, not a command channel or injected toolkit callbacks.
"""
import argparse
import json
import os
from pathlib import Path
import time

CHOICES=['Choice','Alpha','Beta','Gamma','Delta','Echo','Foxtrot','Golf','Hotel','India','Juliet','Other']


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--toolkit',choices=['gtk','qt'],required=True)
    p.add_argument('--report',type=Path,required=True)
    args=p.parse_args()
    log=args.report.open('x',buffering=1)
    def emit(event,**fields):
        log.write(json.dumps(dict(event=event,pid=os.getpid(),uid=os.geteuid(),monotonic=time.monotonic(),**fields))+'\n')
    if args.toolkit=='gtk':run_gtk(emit)
    else:run_qt(emit)
    emit('exit')


def run_gtk(emit):
    import gi
    gi.require_version('Gtk','3.0')
    from gi.repository import Gtk,Gdk,GLib
    emit('version',toolkit='gtk',version='.'.join(str(x) for x in [Gtk.get_major_version(),Gtk.get_minor_version(),Gtk.get_micro_version()]))
    css=Gtk.CssProvider();css.load_from_data(b'* { font-family: Lato; font-size: 20px; } button { min-height: 40px; min-width: 80px; }')
    Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(),css,Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    window=Gtk.Window(title='Tarot GTK fixture');window.set_default_size(340,580)
    box=Gtk.Box(orientation=Gtk.Orientation.VERTICAL,spacing=18);box.set_halign(Gtk.Align.CENTER);box.set_valign(Gtk.Align.CENTER)
    label=Gtk.Label(label='GTK transient fixture');box.pack_start(label,False,False,0)
    button=Gtk.Button(label='Open');box.pack_start(button,False,False,0);window.add(box)
    active=[None]
    def opened(_):
        if active[0] is not None:return
        dialog=Gtk.Dialog(title='GTK child',transient_for=window,modal=True)
        active[0]=dialog;dialog.set_default_size(280,240);dialog.set_resizable(False)
        dialog.add_button('Cancel',Gtk.ResponseType.CANCEL);dialog.add_button('Apply',Gtk.ResponseType.APPLY)
        content=dialog.get_content_area();content.set_border_width(16);content.set_spacing(12)
        entry=Gtk.Entry();entry.set_width_chars(10);content.pack_start(entry,False,False,0)
        combo=Gtk.ComboBoxText()
        for text in CHOICES:combo.append_text(text)
        combo.set_active(0);content.pack_start(combo,False,False,0)
        combo.connect('notify::popup-shown',lambda w,_:emit('menu-parent-size',width=dialog.get_allocated_width(),height=dialog.get_allocated_height()) if w.get_property('popup-shown') else None)
        entry.connect('changed',lambda w:emit('text',field='dialog',text=w.get_text()))
        combo.connect('changed',lambda w:emit('choice',value=w.get_active_text()))
        def response(d,value):
            text=entry.get_text();emit('response',action='apply' if value==Gtk.ResponseType.APPLY else 'cancel',text=text,
                width=d.get_allocated_width(),height=d.get_allocated_height())
            if value==Gtk.ResponseType.APPLY:label.set_text('Saved '+text)
            active[0]=None;d.destroy();button.grab_focus()
        dialog.connect('response',response)
        dialog.show_all();entry.grab_focus();emit('dialog-open',transient=dialog.get_transient_for()==window,modal=dialog.get_modal())
    def close(_window,_event):
        if active[0] is not None:emit('root-close-refused');return True
        emit('root-close');Gtk.main_quit();return False
    button.connect('clicked',opened);window.connect('delete-event',close)
    window.show_all();button.grab_focus();GLib.timeout_add_seconds(180,lambda:(emit('timeout'),Gtk.main_quit(),False)[-1]);emit('ready');Gtk.main()


def run_qt(emit):
    from PySide6 import QtCore,QtGui,QtWidgets
    emit('version',toolkit='qt',version=QtCore.qVersion())
    app=QtWidgets.QApplication([]);app.setApplicationName('Tarot Qt fixture');app.setDesktopFileName('test.tarot.QtDialog');app.setFont(QtGui.QFont('Lato',16))
    active=[None]
    class Parent(QtWidgets.QWidget):
        def closeEvent(self,event):
            if active[0] is not None:emit('root-close-refused');event.ignore()
            else:emit('root-close');event.accept()
    window=Parent();window.setWindowTitle('Tarot Qt fixture');window.resize(340,580)
    layout=QtWidgets.QVBoxLayout(window);layout.addStretch();label=QtWidgets.QLabel('Qt transient fixture');label.setAlignment(QtCore.Qt.AlignmentFlag.AlignCenter);layout.addWidget(label)
    button=QtWidgets.QPushButton('Open');button.setMinimumHeight(44);layout.addWidget(button);layout.addStretch()
    def opened():
        if active[0] is not None:return
        dialog=QtWidgets.QDialog(window);active[0]=dialog;dialog.setWindowTitle('Qt child');dialog.resize(280,240)
        content=QtWidgets.QVBoxLayout(dialog);entry=QtWidgets.QLineEdit();content.addWidget(entry)
        class Combo(QtWidgets.QComboBox):
            def showPopup(self):
                emit('menu-parent-size',width=dialog.width(),height=dialog.height())
                super().showPopup()
        combo=Combo();combo.addItems(CHOICES);content.addWidget(combo)
        buttons=QtWidgets.QDialogButtonBox();cancel=buttons.addButton('Cancel',QtWidgets.QDialogButtonBox.ButtonRole.RejectRole);apply=buttons.addButton('Apply',QtWidgets.QDialogButtonBox.ButtonRole.AcceptRole)
        for b in [cancel,apply]:b.setMinimumHeight(44)
        content.addWidget(buttons)
        entry.textChanged.connect(lambda text:emit('text',field='dialog',text=text));combo.currentTextChanged.connect(lambda text:emit('choice',value=text))
        def finished(value):
            text=entry.text();emit('response',action='apply' if value==QtWidgets.QDialog.DialogCode.Accepted else 'cancel',text=text,
                width=dialog.width(),height=dialog.height())
            if value==QtWidgets.QDialog.DialogCode.Accepted:label.setText('Saved '+text)
            active[0]=None;dialog.deleteLater();button.setFocus()
        buttons.accepted.connect(dialog.accept);buttons.rejected.connect(dialog.reject);dialog.finished.connect(finished)
        dialog.open();entry.setFocus();emit('dialog-open',transient=dialog.windowHandle().transientParent()==window.windowHandle(),modal=dialog.isModal())
    button.clicked.connect(opened);window.show();button.setFocus()
    QtCore.QTimer.singleShot(180000,lambda:(emit('timeout'),app.quit()));emit('ready');app.exec()


if __name__=='__main__':main()
