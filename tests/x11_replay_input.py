"""Scoped XTEST input for a QA process's own nested X11 window.

Finds only windows whose _NET_WM_PID matches the supplied child. Restores focus
and pointer position only if that test window still owns focus on cleanup.
"""
import ctypes as c
import ctypes.util


class X11Input:
    def __init__(self, pid, display=None):
        self.x = c.CDLL(ctypes.util.find_library('X11'))
        self.xtest = c.CDLL(ctypes.util.find_library('Xtst'))
        window = c.c_ulong
        pointer = c.c_void_p
        self.x.XOpenDisplay.argtypes = [c.c_char_p]
        self.x.XOpenDisplay.restype = pointer
        self.x.XDefaultRootWindow.argtypes = [pointer]
        self.x.XDefaultRootWindow.restype = window
        self.x.XInternAtom.argtypes = [pointer, c.c_char_p, c.c_int]
        self.x.XInternAtom.restype = window
        self.x.XGetWindowProperty.argtypes = [pointer, window, window, c.c_long, c.c_long, c.c_int, window, c.POINTER(window), c.POINTER(c.c_int), c.POINTER(window), c.POINTER(window), c.POINTER(c.POINTER(c.c_ubyte))]
        self.x.XFree.argtypes = [pointer]
        self.x.XQueryTree.argtypes = [pointer, window, c.POINTER(window), c.POINTER(window), c.POINTER(c.POINTER(window)), c.POINTER(c.c_uint)]
        self.x.XRaiseWindow.argtypes = [pointer, window]
        self.x.XGetInputFocus.argtypes = [pointer, c.POINTER(window), c.POINTER(c.c_int)]
        self.x.XSetInputFocus.argtypes = [pointer, window, c.c_int, window]
        self.x.XTranslateCoordinates.argtypes = [pointer, window, window, c.c_int, c.c_int, c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(window)]
        self.x.XQueryPointer.argtypes = [pointer, window, c.POINTER(window), c.POINTER(window), c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_uint)]
        self.x.XFlush.argtypes = [pointer]
        self.x.XSync.argtypes = [pointer, c.c_int]
        self.x.XCloseDisplay.argtypes = [pointer]
        self.xtest.XTestFakeMotionEvent.argtypes = [pointer, c.c_int, c.c_int, c.c_int, window]
        self.xtest.XTestFakeButtonEvent.argtypes = [pointer, c.c_uint, c.c_int, window]
        self.display = self.x.XOpenDisplay(display.encode() if display is not None else None)
        assert self.display, 'An accessible X11 display is required'
        self.root = self.x.XDefaultRootWindow(self.display)
        self.window = None
        self.frame_window = None
        self.held = False
        self.pid = pid
        self.old_focus, self.old_revert = self.focus()
        root, child = window(), window()
        rx, ry, wx, wy, mask = c.c_int(), c.c_int(), c.c_int(), c.c_int(), c.c_uint()
        assert self.x.XQueryPointer(self.display, self.root, c.byref(root), c.byref(child), c.byref(rx), c.byref(ry), c.byref(wx), c.byref(wy), c.byref(mask))
        self.old_position = (rx.value, ry.value)
        if mask.value & 256:
            self.x.XCloseDisplay(self.display)
            raise AssertionError('Do not begin synthetic input while button 1 is already held')

    def property(self, window, name):
        atom = self.x.XInternAtom(self.display, name.encode(), 1)
        if not atom:
            return []
        actual, count, after = c.c_ulong(), c.c_ulong(), c.c_ulong()
        fmt, data = c.c_int(), c.POINTER(c.c_ubyte)()
        code = self.x.XGetWindowProperty(self.display, window, atom, 0, 4096, 0, 0, c.byref(actual), c.byref(fmt), c.byref(count), c.byref(after), c.byref(data))
        try:
            assert code == 0
            if fmt.value != 32 or not data:
                return []
            values = c.cast(data, c.POINTER(c.c_ulong))
            return [values[i] for i in range(count.value)]
        finally:
            if data:
                self.x.XFree(data)

    def find(self):
        candidates = [w for w in self.property(self.root, '_NET_CLIENT_LIST') if self.property(w, '_NET_WM_PID') == [self.pid]]
        if not candidates:
            # Private Xvfb has no window manager and no _NET_CLIENT_LIST.
            # Inspect owned PID properties; never fall back to a window title.
            queue = [self.root]
            for _ in range(4096):
                if not queue:
                    break
                node = queue.pop()
                if self.property(node, '_NET_WM_PID') == [self.pid]:
                    candidates.append(node)
                root, parent, count = c.c_ulong(), c.c_ulong(), c.c_uint()
                children = c.POINTER(c.c_ulong)()
                assert self.x.XQueryTree(self.display, node, c.byref(root), c.byref(parent), c.byref(children), c.byref(count))
                try:
                    queue.extend(children[i] for i in range(count.value))
                finally:
                    if children:
                        self.x.XFree(children)
            assert not queue, 'Unexpectedly large X11 window tree'
        assert len(candidates) <= 1, 'Ambiguous test compositor window'
        if candidates:
            self.window = candidates[0]
        return self.window

    def focus(self):
        focus, revert = c.c_ulong(), c.c_int()
        self.x.XGetInputFocus(self.display, c.byref(focus), c.byref(revert))
        return focus.value, revert.value

    def pointer(self):
        root, child = c.c_ulong(), c.c_ulong()
        x, y, wx, wy, mask = c.c_int(), c.c_int(), c.c_int(), c.c_int(), c.c_uint()
        assert self.x.XQueryPointer(self.display, self.root, c.byref(root), c.byref(child), c.byref(x), c.byref(y), c.byref(wx), c.byref(wy), c.byref(mask))
        return {'actual_root':(x.value,y.value),'root_child':child.value,'mask':mask.value}

    def move(self, x, y):
        assert self.window and self.property(self.window, '_NET_WM_PID') == [self.pid]
        rx, ry, child = c.c_int(), c.c_int(), c.c_ulong()
        assert self.x.XTranslateCoordinates(self.display, self.window, self.root, 0, 0, c.byref(rx), c.byref(ry), c.byref(child))
        # Raise only this client's own reparenting frame. Keyboard focus alone
        # does not prevent a second nested window from covering its XTEST target.
        ancestor = self.window
        while True:
            tree_root, parent, count = c.c_ulong(), c.c_ulong(), c.c_uint()
            children = c.POINTER(c.c_ulong)()
            assert self.x.XQueryTree(self.display, ancestor, c.byref(tree_root), c.byref(parent), c.byref(children), c.byref(count))
            if children:
                self.x.XFree(children)
            if parent.value == self.root:
                break
            assert parent.value and parent.value != ancestor
            ancestor = parent.value
        self.x.XRaiseWindow(self.display, ancestor)
        self.frame_window = ancestor
        self.x.XSetInputFocus(self.display, self.window, 2, 0)
        expected = (rx.value + round(x), ry.value + round(y))
        # A new host window can appear beneath an unmoved XTEST pointer.
        # Prime hover only before a press: detouring through the corner during
        # a held drag changes its axis/velocity before the intended motion.
        if not self.held:
            assert self.xtest.XTestFakeMotionEvent(self.display, -1, rx.value + 2, ry.value + 2, 0)
        assert self.xtest.XTestFakeMotionEvent(self.display, -1, *expected, 0)
        self.x.XSync(self.display, 0)
        root, child = c.c_ulong(), c.c_ulong()
        ax, ay, wx, wy, mask = c.c_int(), c.c_int(), c.c_int(), c.c_int(), c.c_uint()
        assert self.x.XQueryPointer(self.display, self.root, c.byref(root), c.byref(child), c.byref(ax), c.byref(ay), c.byref(wx), c.byref(wy), c.byref(mask))
        return {'expected_root': expected, 'actual_root': (ax.value, ay.value), 'root_child': child.value, 'test_window': self.window, 'frame_window':ancestor, 'focus': self.focus()[0]}

    def button(self, pressed):
        if pressed:
            assert not self.held
            assert self.focus()[0] == self.window, 'Stop replay if another window takes focus'
            assert self.pointer()['root_child'] == self.frame_window, 'Stop replay if the pointer is over another window'
        else:
            assert self.held, 'Never release a button this helper did not press'
        assert self.xtest.XTestFakeButtonEvent(self.display, 1, int(pressed), 0)
        self.held = pressed
        self.x.XSync(self.display, 0)

    def close(self):
        if self.held:
            # A focus change must never leave our synthetic drag/button stuck.
            self.xtest.XTestFakeButtonEvent(self.display, 1, 0, 0)
            self.held = False
            self.x.XSync(self.display, 0)
        if self.window and self.focus()[0] == self.window and self.pointer()['root_child'] == self.frame_window:
            self.xtest.XTestFakeMotionEvent(self.display, -1, *self.old_position, 0)
            self.x.XSetInputFocus(self.display, self.old_focus, self.old_revert, 0)
            self.x.XFlush(self.display)
        self.x.XCloseDisplay(self.display)
