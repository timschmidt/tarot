# USB gadget sleep failure and hook

October 30, 2026. The retained `fe800000.usb` `-11` failure from the
[explicit-request audio test](../audio-kernel/README.md) has now been isolated.
Signed revision 60 ships an elogind system-sleep hook that avoids it. With the
hook, ten real RTC-woken deep-sleep cycles succeed in normal Tarot. This
resolves T016 step 5's USB blocker for the current no-host USB-power
configuration. It does not enable automatic sleep or complete T016.

## Cause

The phone's dwc3 controller runs postmarketOS's configfs NCM gadget `g1` in
device mode. The phone currently draws USB power with no USB host attached
(`/sys/class/udc/fe800000.usb/state` is `not attached`). The host's
`172.16.42.x` USB network belongs to a different device, not this phone.
Earlier successful 20-cycle runs had a live USB host link.

On kernel 7.0.0, suspend first tries the bound gadget's soft disconnect. That
fails here, and the controller aborts suspend with `-EAGAIN`. This matches the
upstream change ["usb: dwc3: Abort suspend on soft disconnect failure"](https://patchew.org/linux/20250416100515.2131853-1-khtsai@google.com),
which returns `-EAGAIN` instead of powering down a still-connected controller.
Why soft disconnect fails without a host remains a kernel-level question.

A root probe (`tests/phone_usb_suspend_probe.py`) ran two RTC-woken login1
`SuspendWithFlags(1)` requests in the normal session:

| Gadget | Kernel success | Kernel failure | Result |
| --- | --- | --- | --- |
| Bound, not attached (control) | 0 → 0 | 4 → 6 | deep and s2idle fallback both fail in dwc3 |
| Temporarily unbound | 0 → 1 | 6 → 6 | deep sleep and RTC resume succeed |

The probe restored the original binding.

## Mitigation

`tarot-usb-sleep-hook` (Rust) is installed as
`/usr/libexec/elogind/system-sleep/tarot-usb-gadget`. elogind runs it around
every system sleep.

- **`pre`:** finds configfs gadgets whose controller reports `not attached`.
  It writes their gadget/controller names to root-only
  `/run/tarot-usb-sleep` *before* unbinding them. Attached gadgets are never
  touched. A leftover record from an unrestored sleep refuses a new unbind, so
  failure leads back to the original kernel behavior rather than to lost state.
- **`post`:** rebinds each recorded gadget that is still unbound, keeps any
  binding someone made while asleep, and then removes the record. Malformed
  records or rebinding errors keep the record for a later retry.

Kernel names are validated before any path use. The hook never blocks sleep;
its messages go to elogind's stderr, which this image does not retain in syslog.
Removing the package removes the hook. Unit tests cover these cases with a
temporary sysfs/configfs tree.

## Native acceptance

The r60 package audit verifies:
- the four executables are AArch64
- the hook is the only added file: root:root, mode 0755
- dependencies, PAM configuration and the session entry are unchanged
- there are no installation scripts

The controller installed r60 and started normal (non-diagnostic) Tarot through
real greetd/PAM. It then ran five root `SuspendWithFlags(1)` requests, each
woken by an RTC alarm. Each cycle checked that:
- `g1` was bound to `fe800000.usb` and `not attached` beforehand
- the kernel success counter rose by exactly one, with failures unchanged
- `g1` was rebound and the restore record was gone afterward
- the same compositor process survived

The battery driver reports 15 seconds asleep per resume. A first controller
(failed1) passed all five cycles. Its restoration then failed a self-inflicted
check: the cycle code reused the variable holding the restoration baseline. The
saved before/after baselines were in fact identical, but the restoration path
stopped before its normal login. A real greetd/PAM normal login restored the
session. The corrected check2 then passed five more cycles and full
restoration, with an independent audit of normal Tarot60.

Across both runs, deep-sleep successes rose 1 → 11, and failures stayed at 6
throughout. Compositor PID/start identity was unchanged within each run. Both
compilers pass 288 workspace tests.

| Artifact | SHA-256 |
| --- | --- |
| Canonical source archive | `38d6546e7d247127604d9e66d954417792eeb696eca9ea88f8231bf03449c3eb` |
| Signed APK `tarot-0.1.0-r60.apk` | `af300055385003734a7f757c5b635639c9706ffd5476d845245744d1e5d97521` |
| Probe result | `d508eca9091179766047e7c8b3b1bdea48993cd9c07ce2f4f86d4ccd1b315ec6` |
| Accepted check2 evidence (647 members) | `2df376a1b17cc526c41b93d74649eea16f03670174df65366074a34232bd93fb` |
| Failed1 proof | `5cb580d233c68d58a836e78853d7a8897835c4ae6d0a066d25cb73fe41b5d358` |

## Limits

These cycles were explicit root requests on external USB power, with no audio,
PAM unlock or app checks after resume. They do not repeat the earlier
protected-resume/PAM/app endurance gates on this binary. Not yet tested:
- a USB host attached during sleep
- battery-only sleep
- longer endurance
- the manager's automatic idle action, which is still `ignore`

Next in T016: package and qualify the manager idle-action policy with rollback,
global inhibitors and actual automatic sleep. The underlying kernel soft
disconnect failure should be reported upstream with these observations.
