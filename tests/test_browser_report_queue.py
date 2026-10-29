"""Shared browser producer: input order, bounded polls and failure recovery."""
import shutil
import subprocess
import unittest
from browser_fixture import REPORT_TRANSPORT

NODE=shutil.which('node')

@unittest.skipUnless(NODE,'Node is optional for the browser producer transport test')
class BrowserReportQueueTests(unittest.TestCase):
    def test_serial_delivery_poll_coalescing_and_failure_recovery(self):
        setup="""import assert from 'node:assert/strict';
        let sequence=0;const page='main',documentId='synthetic';
        let active=0,maxActive=0;const calls=[],resolutions=[];
        globalThis.fetch=(_url,options)=>{active++;maxActive=Math.max(active,maxActive);calls.push(JSON.parse(options.body));return new Promise((resolve,reject)=>resolutions.push({resolve:v=>{active--;resolve(v);},reject:e=>{active--;reject(e);}}));};
        const settled=()=>new Promise(resolve=>setImmediate(resolve));
        """
        test="""
        send('input',{fields:{primary:{state:10}}});
        send('input',{fields:{primary:{state:11}}});
        send('heartbeat',{fields:{primary:{state:11}}});
        send('heartbeat',{fields:{primary:{state:12}}});
        send('input',{fields:{primary:{state:12}}});
        assert.equal(calls.length,1);
        resolutions[0].resolve({ok:true});await settled();
        assert.equal(calls.length,2);
        resolutions[1].reject(new Error('synthetic transport failure'));await settled();
        resolutions[2].resolve({ok:true});await settled();
        resolutions[3].resolve({ok:true});await settled();
        assert.deepEqual(calls.map(e=>e.sequence),[1,2,4,5]);
        assert.equal(calls.filter(e=>e.event==='heartbeat').length,1);
        assert.deepEqual(calls.map(e=>e.transportFailures),[0,0,1,1]);
        assert.equal(maxActive,1);assert.equal(active,0);
        send('pageshow');assert.equal(calls.length,5);
        resolutions[4].resolve({ok:true});await settled();
        assert.equal(pendingReports.length,0);assert.equal(reportInFlight,false);
        console.log('PASS: one pending HTTP request; input order retained; polls coalesced; transport failures counted and recovered');
        """
        subprocess.run([NODE,'--input-type=module'],input=setup+REPORT_TRANSPORT+test,text=True,check=True,capture_output=True)

    def test_stalled_request_is_aborted_and_newest_poll_can_arrive(self):
        setup="""import assert from 'node:assert/strict';
        let sequence=0;const page='main',documentId='synthetic';
        const calls=[],timers=new Map();let timerId=0,active=0,maxActive=0;
        globalThis.setTimeout=(fn,delay)=>{assert.equal(delay,3000);timers.set(++timerId,fn);return timerId;};
        globalThis.clearTimeout=id=>timers.delete(id);
        globalThis.fetch=(_url,options)=>{
          active++;maxActive=Math.max(active,maxActive);calls.push(JSON.parse(options.body));
          return new Promise((resolve,reject)=>{
            if(calls.length===1)options.signal.addEventListener('abort',()=>{active--;reject(new Error('deadline'));},{once:true});
            else {active--;resolve({ok:true});}
          });
        };
        const settled=()=>new Promise(resolve=>setImmediate(resolve));
        """
        test="""
        send('window-blur');send('heartbeat');send('heartbeat');send('window-focus');
        assert.equal(calls.length,1);assert.equal(timers.size,1);
        [...timers.values()][0]();await settled();await settled();
        assert.deepEqual(calls.map(e=>e.sequence),[1,3,4]);
        assert.deepEqual(calls.map(e=>e.transportFailures),[0,1,1]);
        assert.equal(maxActive,1);assert.equal(pendingReports.length,0);
        assert.equal(reportInFlight,false);assert.equal(timers.size,0);
        send('input',{fields:{primary:{state:7}}});await settled();
        assert.equal(calls.at(-1).fields.primary.state,7);
        """
        subprocess.run([NODE,'--input-type=module'],input=setup+REPORT_TRANSPORT+test,text=True,check=True,capture_output=True)

if __name__=='__main__':unittest.main()
