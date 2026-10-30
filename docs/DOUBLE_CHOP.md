# Double-chop flashlight investigation

October 30, 2026. The requested end state remains a classifier running on the
IMU's DMP while the application processor is suspended, waking it to toggle the
flashlight once. A persistent Settings switch enables/disables that behavior.
This investigation does not replace that requirement with host polling or
wake-on-motion followed by a CPU classifier. Fx0/MPU-6515M work stays deferred
until M7.3; the current device is the PinePhone Pro.

## Actual development device

The read-only `tests/phone_motion_inventory.py` probe records the kernel, bound
drivers, IIO attributes/events, wake configuration, device-tree interrupt data,
module identities and LED capabilities. It does not upload firmware, change wake
sources, arm events, acquire an IIO buffer or light a flash. The private original
inventory is retained with its probe hash.

On the connected phone, kernel `7.0.0` binds `inv-mpu6050-i2c` to the MPU-6500 at
I²C `4-0068`, exposed as `iio:device3`. The device reports a 50 Hz sampling
frequency. Its available event controls are
`in_accel_x|y|z_roc_rising_value` and `in_accel_x|y|z_roc_rising_en`;
that event is disabled, its threshold is zero, and the parent device's
`power/wakeup` is disabled. No DMP- or firmware-named attribute is exposed.
The registered interrupt is `inv_mpu`, IRQ 82, Rockchip GPIO line 22, level mode.
The device tree identifies an MPU-6500 and supplies interrupt routing; the probe
finds no `wakeup-source` property on that node.

The inspected upstream Linux v7.0 driver implements rate-of-change/wake-on-motion
and a suspend path that retains the accelerometer and arms IRQ wake when both
wake permission and WoM are enabled. It does not expose a DMP program loader or
custom classifier ABI in the inspected driver. This comparison is not proof that
the distribution kernel has identical source or that the actual wake path works.
Capture the distribution patch set and verify its power/interrupt behavior before
changing the installed driver. [Upstream driver](https://github.com/torvalds/linux/blob/v7.0/drivers/iio/imu/inv_mpu6050/inv_mpu_core.c).

## Flash hardware qualification

The actual `white:flash` LED is bound to `sgm3140`. It reports brightness zero,
maximum brightness one, and a 250,000 µs flash timeout. Its selected trigger is
`pattern`. The upstream driver's ordinary brightness path enables torch mode;
its shutdown timer belongs to the separate flash-strobe path. A flash timeout
therefore does not establish a continuous-torch time limit. [Driver source](https://github.com/torvalds/linux/blob/v7.0/drivers/leds/flash/leds-sgm3140.c).

PINE64 identifies the Pro's camera-flash component as AW3641EDNR. A board-specific
report measured excessive torch current and found that the torch/flash selection
signal did not change operation; prolonged use could degrade the LED. These are
concrete reasons to qualify the installed board/driver combination before
exposing an unrestricted flashlight switch. The report is historical, so it does
not establish that every current board has the fault or that this unit does.
[PINE64 component list](https://pine64.org/documentation/PinePhone_Pro/Hardware/),
[hardware investigation](https://xnux.eu/log/069.html#hardware-issues-ive-found).

The capability inventory changed no flash or sensor configuration. No torch-on,
DMP loading, gesture recognition or suspend-wake success is claimed. Preserve
this baseline until the actual chip's current-selection behavior and safe
operating level are established; a software timer alone does not correct an
excessive hardware drive current.

## Implementation gates

1. Map the installed kernel/package/patch identities to the MPU and flash driver
   sources and actual board revision/schematic. Confirm retained IMU supplies,
   interrupt wake routing and the flash chip/control pins.
2. Establish a redistributable DMP firmware/programming route for the MPU-6500,
   with a Rust-owned loader and a supported kernel event interface. Investigate
   a true two-chop classifier, calibration, debounce, reset and stale-event
   handling; hardware WoM alone is not the classifier. Coordinate any driver
   replacement with the existing orientation consumer rather than writing I²C
   registers behind the bound driver's back.
3. Qualify bounded torch operation at a justified current using the correct
   device driver, including camera ownership and failure cleanup. The standard
   login1 Session.SetBrightness API accepts LED devices and is a candidate for
   unprivileged control; its availability is not proof of safe LED operation.
   [login1 interface](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html).
4. Integrate the DMP event and torch policy, preserve lock protection, add the
   persistent switch initially off, and verify disabled-state behavior.
5. Run physical awake/locked/panel-off/deep-suspend double chops, on/off and
   reboot tests, then measure false activation, wake count, latency and energy.
   Record miss rates and thresholds instead of substituting a demonstration
   gesture for the acceptance matrix.

The requested feature remains open under [M4/T015](DEVELOPMENT_PLAN.md#m4-addition-double-chop-flashlight-from-suspend).
