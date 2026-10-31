# Bluetooth with a real test peer

October 31, 2026. The owner provided a Plugable USB Bluetooth 4.0 adapter
(Broadcom BCM20702A0) on the development host as a dedicated test peer. This
resumes physical Bluetooth testing. The result: **Tarot finds the peer by
touch, but pairing is blocked below Tarot.** The phone's Bluetooth controller
cannot open classic connections in either direction.

## Test setup

`tests/bluetooth_peer.py` runs on the host and uses only the adapter it is
given (`5C:F3:70:8C:89:6B`):
- It sets a test alias, `TarotPeer`, and makes the adapter visible and pairable.
- It registers a DisplayYesNo agent that records and accepts numeric-comparison
  codes.
- On exit it removes any pairing with the phone and restores every adapter
  property it changed. An alias equal to the system name is restored as
  BlueZ's default (empty), so no explicit alias is left stored.
- BlueZ's default-agent stack returns the desktop's own agent afterwards.

`tests/phone_bluetooth_pair_replay.py` runs inside a diagnostic Tarot session
on the phone. It drives the Bluetooth page by touch, with targets found by OCR:
Find devices, select the peer, Pair, confirm the six-digit code, Trust,
Connect/Disconnect, and the two-step Forget. Each step is read back
independently from BlueZ by the peer's address. After the run it reads the
code Tarot displayed so it can be compared with the peer's.

## Result (signed revision 64)

1. **Discovery works.** Tarot's Find devices discovered the peer by inquiry.
   BlueZ readback showed the device with alias `TarotPeer`, and a touch on its
   row selected it.
2. **Pairing fails.** A touch on Pair started pairing. Tarot reported "Bluetooth
   action failed; check the device and try again", and the peer never saw a
   connection.
3. **The same failure outside Tarot.** Calling BlueZ's `Device1.Pair` directly
   on the phone, with no Tarot involvement, found the peer (RSSI −72, with its
   full service list) and failed with
   `org.bluez.Error.ConnectionAttemptFailed: Page Timeout`. The earlier JLab
   headset failure (`br-connection-page-timeout`) matches.
4. **The reverse direction fails too.** The phone was made discoverable for a
   bounded window, and btmgmt reported `connectable discoverable`. The host's
   inquiry still never saw the phone, though it found other nearby devices.

The phone's controller can therefore send an inquiry and hear replies, but it
cannot page a device or answer an inquiry. At boot the kernel loads
`brcm/BCM4345C0.hcd` from `linux-firmware-brcm`, which reports itself as
"BCM43455 37.4MHz Raspberry Pi 3+". No board-specific
`BCM4345C0.pine64,pinephone-pro.hcd` exists, and the distribution's
`device-pine64-pinephonepro-nonfree-firmware` only depends on that generic
package. A firmware or clock mismatch is a plausible cause but unproven.
Wi-Fi, which shares the module, works.

Tarot correctly reported the failure and left no pairing or visibility behind.
Host and phone adapters are restored exactly, and an independent post-failure
normal audit passed. Evidence: `artifacts/m4/bluetooth-peer/` (phone proof
`d110f77bbf8d71c965ea894fc1f6c019fdff0059d314cad239b7d00f350357d3`).

## Next steps

Physical pairing, trust/connect and Bluetooth audio acceptance stay blocked on
the platform. The test peer and replay are ready to rerun once the controller
can connect. Candidate investigations, none tried yet, include:
- a Bluetooth patch file matching the PinePhone Pro's AP6255 module and clock
- the controller's UART and sleep-clock configuration in the device tree
- whether the problem reproduces in the stock Phosh session and on a newer
  kernel or firmware
- a low-energy-only pairing path

Report the platform issue upstream once the stock-session check is done.
