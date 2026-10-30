# Persistent automatic-sleep settings

October 30, 2026. Signed revision 59 adds the T016 step-1 user policy. The
Power page has an **Automatic sleep** section (off by default). It sets a delay
after protected screen-off, and whether external power is allowed. Settings are
saved in settings schema 6. The session idle publisher now also requires this
setting, plus its elapsed delay. The normal launcher still does not start the
publisher, and the manager keeps `IdleAction=ignore`. No automatic or requested
kernel sleep occurs. This does not complete T016 or M4.
[Contract](../../SESSION_IDLE.md#power-settings) · [r58 publisher evidence](../session-idle-native/README.md).

## Native acceptance

The same root-only controller and independent GIO observer as revision 58 run
two real diagnostic greetd/PAM logins on VT7, with the phone on USB power.
Before each login, the controller resets the QA preferences file. Settings are
changed through the private control socket while unlocked. The publisher, power
key, panel driver and PAM are all real.

**External power allowed by the diagnostic override:**

- The default settings load as off with a 60-second delay. Enabling automatic
  sleep with an immediate delay reproduces the full r58 sequence: automatic
  off with a held audio block, last-audio release, wake and PAM, a manual power
  key, and locker replacement while off.
- A 10-second delay is then saved, and the power key turns the panel off. The
  compositor reports `Delay` with the proven off counter, and direct property
  reads keep returning false. The last false read is at 10.007 s after the
  request. The first true read is at 10.222 s, with no further input. The
  readiness timer published it.
- An explicit false precedes session removal. The controller then checks the
  saved file: schema 6 with `[display.sleep]`, automatic on, 10 s, external
  power off.

**Battery-only policy:**

- Automatic and manual power-off report `ExternalPower`, and the hint stays false.
- After PAM, turning the setting off makes the next power-off report
  `Disabled`, and the hint stays false.
- After PAM again, enabling the persistent **external power** setting makes the
  next power-off `Ready`, and the hint goes true. It is cleared before session
  removal.

Kernel suspend counters are unchanged in both sessions, and no Suspend method
is called. Exact audio, preference, policy, network, backlight and feedback
restoration pass. The independent normal audit verifies the following for
normal Tarot59: loaded hashes, Panfrost, diagnostics off, no session-idle
arguments, `IdleHint` false and `IdleAction` ignore.

## Host scope

Rust 1.98.1 and minimum 1.89.0 each pass 284 workspace tests, with nine ignored.
New unit tests cover:

- Delay ordering after every safety and power condition
- Default-off bounded settings
- A schema-6 migration that preserves the original file and rejects a v5 file
  carrying the new section, out-of-range values and unknown keys
- Concurrent preference merging

A rendered Power-page test covers the unavailable state, one command per touch,
quiet repaint before confirmation, the not-configured, unknown-power and error
lines, and the hidden section under managed idle. Both compilers' binaries pass
the private idle matrix. Its nested case now proves the same publisher reports
`Disabled` without the saved setting and `Active` with it. Owned-code strict
Clippy and formatting pass.

The clean Alpine build reuses the 446 locked registry packages. The package
audit passes: only the four executables and README change from r58.

| Artifact | SHA-256 |
| --- | --- |
| Canonical source archive | `c223bd364b0ccb085e63e4e765072cb1b065cd0d1e38b3045d808cec5e6d7e87` |
| Signed APK `tarot-0.1.0-r59.apk` | `31e09d6b9a10154c52ae56dea57f724ac24210eb46a9b6b4a5e637797e9177f7` |
| Accepted native payload | `77784c5387074f9e14e2f7dabd8351dec238a899a21d2eb3f3951a09d1aeba85` |
| Accepted native evidence (656 members) | `fb03ac528447bd0901fc50155336a5b23c93803b319558d1a58406a82f42a04a` |
| r59 failed1 proof | `f6e3636bfbc7b2f6cd3abd57ba152b7f9b6a957d3710ab04a276cd6acfa2f624` |

## Retained failures

- **First private rerun.** It read display settings from a redacted protected
  status. The nested assertion now uses readiness instead.
- **r59 failed1.** The first native controller passed external mode. The battery
  login then loaded the external run's saved sleep settings, as persistence
  should, and failed the replay's default-settings baseline. The controller now
  verifies that saved file and resets the QA preferences before each mode.
- **Delay timing method.** Failed1's delay check used observer signal times.
  Those are dispatch times and give only an upper bound, so check2 bounds the
  transition with direct property reads.

No runtime code or package changed between attempts. Failed1's restoration and
its post-failure normal audit passed.

## Limits

The settings were changed through the private control path, not by finger on the
Power page. Physical touch review of the new section remains open, alongside:

- T016 manager-policy packaging and rollback
- Global-inhibitor qualification
- The retained `fe800000.usb` `-11` kernel failure
- Actual RTC-woken automatic sleep on battery and external power

Physical Bluetooth stays deferred.
