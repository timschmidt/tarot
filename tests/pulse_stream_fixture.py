#!/usr/bin/env python3
"""Owned null-device stream with explicit cork control; no physical audio device.

Playback writes zero PCM. Recording discards an owned null source/monitor without
copying or inspecting samples. This is QA, never runtime shell code.
"""
import argparse
import ctypes as C
import ctypes.util
import json
import sys
import threading
import time


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--server', required=True)
    p.add_argument('--device', required=True)
    p.add_argument('--record', action='store_true')
    a = p.parse_args()
    assert a.server.startswith('unix:') and a.device.startswith('tarot_qa_')
    lib = C.CDLL(ctypes.util.find_library('pulse') or 'libpulse.so.0')
    ptr, integer, size = C.c_void_p, C.c_int, C.c_size_t
    class Sample(C.Structure):
        _fields_ = [('format', integer), ('rate', C.c_uint32), ('channels', C.c_uint8)]
    class Buffer(C.Structure):
        _fields_ = [(n, C.c_uint32) for n in ['maxlength', 'tlength', 'prebuf', 'minreq', 'fragsize']]
    Request = C.CFUNCTYPE(None, ptr, size, ptr)
    Success = C.CFUNCTYPE(None, ptr, integer, ptr)
    def function(name, result, args):
        f = getattr(lib, 'pa_' + name); f.restype = result; f.argtypes = args; return f
    new_loop = function('threaded_mainloop_new', ptr, [])
    api = function('threaded_mainloop_get_api', ptr, [ptr])
    start = function('threaded_mainloop_start', integer, [ptr])
    lock = function('threaded_mainloop_lock', None, [ptr])
    unlock = function('threaded_mainloop_unlock', None, [ptr])
    stop = function('threaded_mainloop_stop', None, [ptr])
    free = function('threaded_mainloop_free', None, [ptr])
    context_new = function('context_new', ptr, [ptr, C.c_char_p])
    connect = function('context_connect', integer, [ptr, C.c_char_p, integer, ptr])
    context_state = function('context_get_state', integer, [ptr])
    context_disconnect = function('context_disconnect', None, [ptr])
    context_unref = function('context_unref', None, [ptr])
    stream_new = function('stream_new', ptr, [ptr, C.c_char_p, C.POINTER(Sample), ptr])
    stream_state = function('stream_get_state', integer, [ptr])
    index = function('stream_get_index', C.c_uint32, [ptr])
    is_corked = function('stream_is_corked', integer, [ptr])
    set_write = function('stream_set_write_callback', None, [ptr, Request, ptr])
    set_read = function('stream_set_read_callback', None, [ptr, Request, ptr])
    write = function('stream_write', integer, [ptr, ptr, size, ptr, C.c_int64, integer])
    peek = function('stream_peek', integer, [ptr, C.POINTER(ptr), C.POINTER(size)])
    discard = function('stream_drop', integer, [ptr])
    playback = function('stream_connect_playback', integer, [ptr, C.c_char_p, C.POINTER(Buffer), integer, ptr, ptr])
    record = function('stream_connect_record', integer, [ptr, C.c_char_p, C.POINTER(Buffer), integer])
    cork = function('stream_cork', ptr, [ptr, integer, Success, ptr])
    op_cancel = function('operation_cancel', None, [ptr])
    op_unref = function('operation_unref', None, [ptr])
    disconnect = function('stream_disconnect', integer, [ptr])
    unref = function('stream_unref', None, [ptr])
    zeros = C.create_string_buffer(65536)
    callback_failed = threading.Event()
    @Request
    def write_ready(stream, length, _):
        # Bounded by a small requested buffer; unexpected large requests fail.
        if length > len(zeros) or write(stream, zeros, length, None, 0, 0) != 0:
            callback_failed.set()
    @Request
    def read_ready(stream, length, _):
        data, count = ptr(), size()
        if peek(stream, C.byref(data), C.byref(count)) != 0 or discard(stream) != 0:
            callback_failed.set()
    loop, context, stream = new_loop(), None, None
    started = False
    def locked(call):
        lock(loop)
        try: return call()
        finally: unlock(loop)
    def wait(predicate):
        deadline = time.monotonic() + 4
        while not locked(predicate):
            assert time.monotonic() < deadline and not callback_failed.is_set()
            time.sleep(.01)
    def emit(**fields):
        print(json.dumps(fields), flush=True)
    try:
        assert loop
        context = context_new(api(loop), b'Tarot null-stream QA')
        assert context and start(loop) == 0
        started = True
        assert locked(lambda: connect(context, a.server.encode(), 1, None)) == 0
        wait(lambda: context_state(context) == 4)
        spec = Sample(3, 16000, 1)  # PA_SAMPLE_S16LE, no platform endian assumption.
        stream = locked(lambda: stream_new(context, b'Tarot null-stream QA', C.byref(spec), None))
        assert stream
        buffer = Buffer(65536, 4096, 0, 1024, 1024)
        if a.record:
            locked(lambda: set_read(stream, read_ready, None))
            code = locked(lambda: record(stream, a.device.encode(), C.byref(buffer), 1))
        else:
            locked(lambda: set_write(stream, write_ready, None))
            code = locked(lambda: playback(stream, a.device.encode(), C.byref(buffer), 1, None, None))
        assert code == 0
        wait(lambda: stream_state(stream) == 2)
        emit(event='ready', index=locked(lambda: index(stream)), corked=bool(locked(lambda: is_corked(stream))))
        for line in sys.stdin:
            value = json.loads(line)
            if value['command'] == 'exit': break
            assert value['command'] == 'cork' and isinstance(value['value'], bool)
            finished = threading.Event(); success = []
            @Success
            def done(_, ok, __):
                success.append(ok); finished.set()
            operation = locked(lambda: cork(stream, int(value['value']), done, None))
            assert operation
            try:
                assert finished.wait(3) and success == [1] and not callback_failed.is_set()
            finally:
                locked(lambda: (op_cancel(operation), op_unref(operation)))
            wait(lambda: bool(is_corked(stream)) == value['value'])
            emit(event='corked', value=value['value'])
        assert not callback_failed.is_set()
    finally:
        if started: lock(loop)
        try:
            if stream: disconnect(stream); unref(stream)
            if context: context_disconnect(context); context_unref(context)
        finally:
            if started: unlock(loop); stop(loop)
            if loop: free(loop)


if __name__ == '__main__': main()
