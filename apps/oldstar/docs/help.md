# OldStar help

OldStar turns an old tablet or phone into a remote control, with nothing to install on it: the
PC running OldStar serves a web page on the local network, and the tablet opens it in its
browser. From that page you control a **Yamaha CL/QL** or **Midas M32 / Behringer X32** console,
the buttons of **CueStar** and **JingleStar**, and you can see (and touch) **PC windows** such as
Lake Controller. A console is optional: without one, OldStar only offers the Apps and Windows
tabs.

## Getting started

1. Start OldStar on the PC. On first launch, Windows asks to allow OldStar through the firewall:
   accept, otherwise tablets cannot reach it.
2. Connect the tablet to the same network (Wi-Fi) as the PC.
3. On the tablet, open the browser and type the address shown in large type in the
   **Connection** tab (for example `http://192.168.1.20:8080/`), or scan the **QR code**.
4. At the top right of the page, **Online** (green) means the tablet is talking to the PC.
5. In **Settings**, choose the language, the console (or **None**) and its IP address, then
   **Save**.

Tip: in Safari, **Share → Add to Home Screen** adds OldStar like an app; it then opens full
screen, without the address bar.

Supported devices: iOS 9 and later (iPad 2, iPad 3, iPad mini 1, iPhone 4S, iPod touch 5) and
Android 4.4 and later (stock browser or Chrome).

## The PC window

### Top bar

| Item | Purpose |
|---|---|
| **Connection** | Address to type on the tablet, QR code, console and device status |
| **Log** | Every OldStar message, time-stamped |
| **Settings** | Language, page port, console and its address, free choice of windows |
| Book | Opens this help (**F1** key) |
| **Quit** | Stops OldStar |

Closing the window stops OldStar: tablets then show **Offline**.

### Connection

- The address to type on the tablet, in large type, and its **QR code**.
- **Copy address** and **Open the page on this PC** (check the page without a tablet).
- **Network adapter**: if the PC has several (Wi-Fi, cable, virtual adapter…), pick the one on
  the tablet's network. **Refresh addresses** after plugging in a cable.
- Console status (connected or offline, model) and the number of connected devices.

### Log

Start-up, addresses, devices connecting, console, errors, touches received. **Copy** copies the
whole log to the clipboard (useful to report a problem), **Clear** empties it. It is also
written to `oldstar.log`, in the settings folder. Log messages are in French.

### Settings

| Setting | Purpose |
|---|---|
| Language | English or French, for this window and for the tablet page |
| Page port | HTTP port of the page (8080 by default) |
| Console | **None**, Yamaha CL/QL or Midas M32 / X32 |
| IP address and port | Where to reach the console |
| PC windows | The page may show any open window, or only the shortcuts |

The language applies as soon as you **Save**: open tablet pages reload in the new language. The
port and the console choice take effect at the next launch; a new console address is used right
away. Everything is saved in `oldstar.json`, in OldStar's settings folder (its path is shown at
the bottom of the tab).

## On the tablet

### Top strip

The **Console** (hidden when the console is **None**), **Apps**, **Windows** and **Help** tabs,
then the link status:

| Light | Meaning |
|---|---|
| **Online** (green) | Real-time link |
| **Online (slow)** (green) | The network or browser blocks real time: slower link that works everywhere |
| **Connecting…** (yellow) | Looking for the PC |
| **Offline** (red) | PC unreachable; the page reconnects by itself and resumes where it was |

On a very small screen, the status is reduced to its coloured dot.

### Console

- One strip per input channel, or per mix (**Inputs / Mix** buttons): name, meter, fader, dB
  value and **ON** button. Strips scroll left and right.
- Move the fader by sliding a finger from anywhere on the strip: touching it does not make the
  level jump. Its travel follows a console fader (−∞ at the bottom, 0 dB at three quarters,
  +10 dB at the top).
- **EQ / Sends** opens a channel's details: EQ curve, band points to drag (frequency and gain),
  **GAIN**, **FREQ** and **Q** encoders turned by sliding a finger up or down, LOW and HIGH band
  types, **HPF**, **EQ ON** and **FLAT**. Below, the sends to each mix with their ON button.
- **SENDS ON FADER**: pick a mix (a bus on the M32); faders and ON buttons then set each
  channel's send to that mix. A yellow banner shows the mode; **Back to faders** returns to the
  main faders.
- Changes made on the console or on another tablet show up at once. Meters only run while the
  Console tab is shown, to spare Wi-Fi and battery.

### Apps

The CueStar and JingleStar buttons (GO, Stop, Pause, previous or next cue, Black…), in large
size. The green light shows the app is answering. For CueStar: the standby cue, the media on
screen and the remaining time.

### Windows

- Pick a **shortcut** (Lake Controller, CL/QL editor, zoomed areas) or any open PC window
  (**Refresh** updates the list). A window minimised to the taskbar cannot be shown: it is
  flagged.
- Touching the image clicks in the window: short tap = click, two quick taps = double-click,
  long press = right-click, slide = drag and drop. Slide with **two fingers** to scroll (mouse
  wheel on a PC).
- **Click: foreground** (real mouse clicks, the window comes to the front) or **Click:
  background** (the window gets the click without coming to the front). Some apps ignore
  background clicks: switch to foreground.
- **Actual size / Fit**: image at its size, or fitted to the screen. **Full screen** hides
  everything but the image; **✕** in the corner leaves it.
- **◀ Close**, always at the top, goes back to the list (Esc key on a PC).

Window capture works on Windows and macOS (not on Linux). On a Mac, OldStar needs two
permissions, in **System Settings › Privacy & Security**: **Screen Recording** to see windows
(then restart OldStar), and **Accessibility** for touches to click. macOS asks on first use.

### Help

This help, readable on the tablet.

## Trying without a console

- `oldstar --simulateur` starts a fake Yamaha QL1 on the PC and connects to it (faders moving by
  themselves, fake meters).
- `oldstar --simulateur-m32` does the same with a fake M32.
- `oldstar --console aucune` starts without a console, `oldstar --lang en` (or `fr`) chooses the
  language for this launch.
- `oldstar --sans-fenetre` runs without a window; the log is shown in the console.

## Troubleshooting

| Symptom | Check |
|---|---|
| The tablet cannot open the page | Same network as the PC? Right adapter in **Connection**? OldStar allowed through the firewall? |
| **Offline** on the tablet | Is OldStar still running? Is the tablet's Wi-Fi asleep? |
| Console **offline** | IP address and port in **Settings**; is the console on the same network? |
| No Console tab | The console is set to **None** in **Settings** |
| A window stays black | It is minimised to the taskbar (or the Dock): restore it on the PC. On a Mac: Screen Recording permission |
| Touches do nothing | Try **Click: foreground**. On a Mac: Accessibility permission |

The **Log** nearly always tells what is going on.
