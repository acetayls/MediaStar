# SweepStar

SweepStar measures a sound system or a room: frequency response, phase, impulse response and
delay, with a sweep (swept sine). A band analyser (RTA) shows the microphone spectrum live.

## Connecting

- **Output**: the interface output to the system under test (desk, amplifier, speaker).
- **Microphone**: a measurement microphone on an input of the same interface.
- **Reference loop** (recommended): a cable from an interface output (the same as the
  signal) to another input. The measurement uses it to cancel the interface's latency and
  colour: the displayed delay is then the true time of flight to the microphone.

Choose the interface, signal outputs, microphone input and loop input in **Preferences**.

## Setting the level

Start **pink noise** (left panel, Play) and raise the level until the system is clearly
audible, keeping the microphone meter well away from red. The default level is −20 dBFS.
**Esc** or **STOP** cuts the signal at any time.

## Measuring

Set the sweep in **Preferences**: length (1 to 16 s, 3 s by default: longer = less noise),
frequencies and tail (silence recorded afterwards, for the reverberation). Then click
**MEASURE**, at the top of the Measurements panel.

- **Magnitude**: smoothing from 1/1 to 1/48 octave (menu at the top right).
- **Phase**: the peak delay is removed, to read the system's own phase.
- **Impulse response**: as ETC (dB) or linear.
- **Delay**: in milliseconds and metres, from the air temperature you set.
- **Zoom on peak**: shows the impulse response from 1 ms before to 4 ms after the peak.

**Graphs**: wheel = vertical zoom, **Ctrl + wheel** (or Shift + wheel, or pinch) = frequency (or
time) zoom, drag = move, double-click = full view. Magnitude and phase zoom together.

Without a reference loop, a warning reminds you that the delay includes the interface's
latency.

## Saved measurements

The **Measurements** panel, on the right, keeps your measurements like an analyser: **Capture
measurement** (Sweep & IR view) or **Capture RTA** (RTA view) adds it to the list and writes it
straight away to the measurements folder (Documents/SweepStar/Mesures by default, **Folder…**
to change it). Tick to show and overlay, click the swatch to change colour, rename in the
field (24 colours or a free colour). The Transfer view captures the transfer function the same
way. The **⋯** menu exports to **WAV** (impulse response, 32-bit float) or **REW text**
(frequency, magnitude, phase, with the chosen smoothing) and deletes the file. **Import…** or
drag and drop adds `.ssm` files. Shown measurements stay shown at the next launch. **»**
collapses the panel.

## RTA

Microphone spectrum in 1/1 to 1/48 octave bands, A, C or Z weighting, fast, medium or slow
averaging, and peak hold. **Curve** draws a sliding band of the chosen width (one point every
1/48 octave), without the steps of **Bars**; **Smoothing** softens the spectrum further (1/24
to 1/1 octave). **Bars** are solid bars, one per band.

**Calibrate…**: fit a calibrator (94 or 114 dB SPL at 1 kHz) on the mic and click
**Calibrate**; the RTA and the overall level (LZ, LA or LC) switch to dB SPL. The calibration
holds for this mic, this channel and this preamp gain; it can also be set by hand or removed.

## Transfer

Live transfer function between the **reference loop** and the **mic**, with any signal (pink
noise, music): magnitude, phase and coherence. **Find delay** aligns the reference with the mic
(time of flight, also shown in metres); the delay can also be set by hand. FFT size (4096 to
65,536), averaging (1 to 64 blocks or infinite), smoothing and **Reset**. **Auto track** looks for the delay every second (cross-correlation of
the last 2.7 s of reference and mic, GCC-PHAT) and corrects it when it moves by at least one
sample; it needs a broadband signal (pink noise, music). A loop input must be
chosen in Preferences.

## Panels

**«** collapses the Signal panel on the left, **»** the Measurements panel on the right; the
strip left behind reopens them.

## Playlist

Double-click to play, drag to reorder. Click or drag the position bar to move within the
track. The loop button at the end of a row (or **Loop** for the selected track) repeats that
track alone.

## Shortcuts

- **Esc**: stops the signal and the measurement.
- **F1**: opens or closes the help.
