# Letting the browser reach a USB printer (Linux)

WebUSB can only talk to a device the operating system lets it have. On Linux two
separate things get in the way, they produce different errors, and they have
different fixes — so check which one you actually have.

Find the printer first:

```bash
lsusb                      # e.g. Bus 001 Device 005: ID 0471:0055 USB printer
```

## 1. "Access denied" when opening the device

The raw node under `/dev/bus/usb` is root-owned, so your user cannot open it.
Grant access with a udev rule (substitute your own vendor/product id):

```bash
sudo tee /etc/udev/rules.d/99-escpos-webusb.rules >/dev/null <<'RULE'
# Let the logged-in user talk to this USB printer from a browser (WebUSB).
SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="0055", MODE="0660", TAG+="uaccess"
RULE

sudo udevadm control --reload-rules
sudo udevadm trigger
```

Then **unplug and replug the printer** (the rule applies when the device appears).

## 2. "…is holding this printer" when claiming the interface

Some printers expose a vendor-specific interface, and nothing claims those — they
just work. Others expose the USB **printer class (07)**, and the kernel's `usblp`
driver grabs them. Check:

```bash
# does an interface report class 07 with usblp bound?
for i in /sys/bus/usb/devices/*/; do
  [ "$(cat $i/idVendor 2>/dev/null)" = "0471" ] || continue
  for f in $i*:*/; do
    echo "$(basename $f): class=$(cat $f/bInterfaceClass) driver=$(basename $(readlink $f/driver 2>/dev/null))"
  done
done
```

Chrome will usually detach `usblp` itself once it has permission, so try
printing again after step 1 before doing anything else. If it still fails,
unbind the driver from that one interface (`1-9:1.0` below is the interface id
the command above printed):

```bash
echo -n '1-9:1.0' | sudo tee /sys/bus/usb/drivers/usblp/unbind
```

That lasts until the printer is replugged. To make it stick, tell `usblp` to
ignore this device permanently:

```bash
sudo tee /etc/udev/rules.d/98-escpos-no-usblp.rules >/dev/null <<'RULE'
# Keep usblp off this printer so a browser can claim it (WebUSB).
ACTION=="add", SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="0055", \
  RUN+="/bin/sh -c 'echo -n $kernel > /sys/bus/usb/drivers/usblp/unbind'"
RULE
```

> **Do not do this on a device that prints for real.** Unbinding `usblp` removes
> `/dev/usb/lp0`, which is exactly how a backend prints. This is for a designer's
> workstation — the machine running the browser — not for a machine whose job is
> to print.

## Windows

The printer class is claimed by `usbprint.sys`. Replace it with WinUSB using
[Zadig](https://zadig.akeo.ie/). Same caveat: it takes the device away from
normal printing on that machine.

## Why this is not as bad as it looks

It is a one-time setup on **one** machine — the workstation of whoever designs
tickets. It is not something end users or printing devices ever do.
