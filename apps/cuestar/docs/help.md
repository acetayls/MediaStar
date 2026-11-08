# CueStar help

CueStar sends videos, images and sounds to an output screen (projector, stage screen, video
desk). The control screen, where you work, and the **output**, which shows the picture, run as
two separate programs: a busy control screen never delays a frame.

Three modules share the screen: the **Cuelist** (a running order of media fired with GO, which
can chain on their own), the **Slideshow** (Diaporama, a folder of photos and videos) and the
**Presentation** (Présentation, a PowerPoint file). The **Text** panel sits alongside to keep the written running order in view.

## Getting started

1. At startup the output opens full screen on the second display (in a window if there is only
   one). The bottom bar reads **Sortie : 1920×1080 @ 60.00 Hz** once it is ready.
2. In the **Cuelist**, click **Ajouter des médias** (Add media) or drop files on the list.
3. Double-click a cue, or click it to put it on standby and press **Space** (GO).
4. **Esc** or the red **NOIR** button cuts the picture at any time.
5. Right-click a cue → **Réglages…** (settings) to set its rotation, in and out points, fades
   to black, volume and equalizer.

Everything is kept automatically: when CueStar starts again, the session is as you left it.
**Enregistrer** (Save) writes the project to a `.cuestar` file.

Formats: videos (MP4, MOV, MKV, AVI, WebM, MXF, MPEG, TS, WMV…), images (JPEG, PNG, BMP, GIF,
TIFF, WebP) and sounds (MP3, WAV, FLAC, AAC/M4A, OGG). Decoding goes through libmpv.

## The screen

### Top bar

| Item | Role |
|---|---|
| Project name | Menu: New, Open, Save as, Rename |
| **Enregistrer** | Saves the project (**Ctrl+S**) |
| **Cuelist**, **Diaporama**, **Présentation** | Switches module; a hidden module keeps playing |
| **Texte** | Shows or hides the Text panel |
| **? Help** | Opens this help (**F1**) |
| **Préférences** | Tabbed window: output screen and engine, audio, test patterns, 4-corner mapping |
| **Monitoring** | Floating window showing the picture sent to the output |
| **Télécommandes** | OSC, MIDI, Yamaha and Midas consoles, mappings, shortcuts |
| **NOIR** | Cuts the picture: output to black (**Esc**) |

### Bottom bar

- **Sortie** (output): green when the picture goes through libmpv, orange when libmpv is
  missing (the output then shows a test pattern), red when the output stopped (**Relancer la
  sortie** restarts it).
- mpv version and current decoder (GPU or CPU).
- **Dropped / delayed frames**: frames the output could not show on time, added up over every
  media since the last reset. Hover for details. To start again from zero: the **↺** button, a
  right-click on the counter → **Reset**, the OSC address `output/drops/reset` or a remote.
- Media frame rate, CPU and GPU load.

## The Cuelist

A running order of cues: each one is a media (video, image, sound), or **black**. The
**standby** cue plays on GO. It replaces the former Player (Lecteur): a project that had a
playlist in the Player finds it at the end of the running order, as chained cues with its
transition.

- **Ajouter des médias** (add media): files, a folder, or **Un cue Noir** (a black cue, cut
  with the cue's transition). You can also drop files and folders on the list.
- **GO** (or **Space**) plays the standby cue and puts the next one on standby. If the current
  cue is paused, GO resumes it.
- **⏮ / ⏭** (or **Up / Down arrow**) change the standby cue without playing it.
- **Pause** (or **P**) pauses and resumes the playing cue. **■ Arrêter** cuts the picture.
- Click a cue: puts it on standby. Double-click: plays it. Drag a cue to reorder.

### The media on air

The top box shows the cue on air: its name, its **duration**, the elapsed time, a playback bar
(click or drag to move in the media) and the time left. It turns amber when paused and reads
**FIN DU MÉDIA** when a cue that stops at its end keeps its last frame: GO then plays the
standby cue.

### Chaining, looping

- The orange arrow at the start of a row: at the end of this cue, the next one **plays on its
  own**. The grey bar: at the end, we wait for GO, with the last frame on screen or black
  depending on the cue's **À la fin** setting (**Garder la dernière image** / **Passer au noir**).
- The loop icon at the end of a row **loops** the cue. Unticking it while the cue plays lets the
  current pass run to its end; then the cue chains to the next one or stops on its last frame,
  depending on its arrow.
- **Boucler la conduite** (loop the running order): after the last cue, the first one plays
  again (or waits for GO).
- **Standby cue transition**: **Coupe** (cut), **Noir** (fade to black then up) or **Fondu
  enchaîné** (crossfade), with its duration. Each cue keeps its own; **À tous** gives it to the
  whole running order.

### Standby preview

On the right, the preview shows the cue the next GO will play, rotated as on the output. You can
move around in it without touching the output: click or drag on the bar under the picture,
**▶** plays the preview (silently), **⏮ / ⏭** step one frame back or forward. The in and out
marks are drawn on the bar. **Aperçu** (above the running order) shows or hides it; the handle
between the two sets its width.

### Right-click on a cue

| Command | Effect |
|---|---|
| **Lancer maintenant** | Plays the cue |
| **Réglages…** | Opens the cue settings (see [Editing an item](#editing-an-item)) |
| **File information…** | File details, like MediaInfo (see [File information](#file-information)) |
| **Renommer…** | Renames the cue (Enter to confirm) |
| **Retirer de la conduite** | Removes the cue |

A cue with settings (rotation, points, fades, sound) carries the **✎** mark.

## Editing an item

Right-click a cue → **Réglages…**. At the top: its name, its media, its **transition** and what
happens **at the end** (wait for GO, or go on to the next cue). The rest follows JingleStar's
jingle editor. Settings are saved with the project.

- **Rotation**: 0°, 90°, 180° or 270° clockwise, for this file only (a portrait video, a screen
  on its side). The preview rotates too.
- **Volume** of the item's sound, in dB, and play **once** or **looped**.
- **Fades to black**: length of the fade in (from black) and of the fade out (to black),
  picture and sound together.
- **Strip**: thumbnails of the media over its whole length. Drag the **in** and **out** handles
  to trim the start and the end; fades show as a veil on the strip. While dragging, the preview
  shows the picture at that point.
- **Preview**: the same navigation as the standby preview, to check an in point to the frame.
- **Equalizer**: low cut (12 or 24 dB/oct), low shelf, two bell bands, high shelf, as in
  JingleStar. The sound's waveform shows behind the curve.
- If the cue is on air, volume, equalizer, fades, loop and rotation change at once, without
  cutting the picture.

### Still images

For an image, the editor offers the **display time**: **Until the next media** (the image stays
until the next GO) or **For** a number of seconds, after which the cue is over (and chains if
set to). A fade out
on an image needs a display time.

## File information

Right-click a cue → **File information…**. The window details the container
and every video, audio and subtitle track: codec, profile, bit rate, resolution, frame rate,
scan type, colour space and range, channels, sample rate, language, metadata.

- **Search** filters the rows (for example "bit rate" or "HDR").
- **Show everything** switches to MediaInfo's complete view: every field, in every form.
- **Copy** puts the report on the clipboard as text, to send to a technician.

The report comes from the MediaInfo library when it is installed (`MediaInfo.dll` in CueStar's
`mpv` folder on Windows). Without it, CueStar shows what libmpv knows about the file. The file
is read in a separate program: a damaged file cannot crash the control screen.

## The Slideshow

Plays a folder of photos and videos.

- **Dossier** (folder): the folder to play; **Relire** picks up added or removed files.
- **Include subfolders**, **photo duration**, **fit** (whole picture with black bars, fill the
  screen by cropping, or automatic) and **videos with sound** (played in full).
- **Space** plays or pauses, **← / →** go to the previous / next media.
- **Aléatoire** reshuffles on every round, **Tout boucler** starts over.

Changes apply at once, without restarting the slideshow.

## The Presentation

Shows a PowerPoint presentation (**.pptx**) or a **PDF** on the output, without PowerPoint:
CueStar draws each slide (or each PDF page) itself at the output resolution.

- **Choisir…** (choose) or drop a .pptx or .pdf file; **Relire** reloads the file after a change in
  PowerPoint.
- The **thumbnails** show every slide: click to select, double-click to show it on the output.
  Hidden slides are skipped.
- **LANCER** (go) shows the selected slide, then **SUIVANTE** (next, **Space** or **→**) and
  **←** move from slide to slide; **Aller à** (go to) jumps to a number. The next slide is
  prepared in advance.
- **Transition**: **Coupe** (cut), **Noir** (fade to black then up) or **Fondu enchaîné**
  (crossfade).
- **Esc** goes to black; **LANCER** shows the slide again.
- **B** or **period**: black screen, then back to the slide. **F5** starts from the first slide,
  **Shift+F5** from the selected slide; **Home** and **End** go to the first and last.

**Presentation remote** (PowerPoint-style clicker): works as is, no setup. **Page Down / Up**, the
black-screen button and F5 work in every tab as soon as a presentation is on screen, and also when
the output window has focus.

Text, bullets, shapes, pictures, tables, SmartArt and simple charts are drawn; animations are not:
an embedded video shows as its preview picture.

## The Text panel

The written running order, next to the current module: bold, italic, underline, strikethrough,
colours, size. **Importer…** brings in an existing text. It is saved with the project.

## Video and sound output

**Préférences** opens the output settings, in tabs (Écran et moteur, Audio, Mires de test,
Mapping 4 coins):

- **Output screen**: automatic (first secondary screen) or a given screen; **output in a
  window** when there is only one screen.
- **Decoding**: automatic, GPU or CPU. **Fallback renderer** and **light rendering** for a small
  machine.
- **Picture sync**: **Mélange d'images** (frame blending, recommended and chosen by default)
  works like Automatic, and when the video frame rate does not fit the screen's (24 fps on
  60 Hz) it blends the two neighbouring frames instead of repeating one: motion stays even. It
  does nothing when the rates match, nor on test patterns (so as not to skew the measurement).
  The other choices: **Automatic, no blending**, **On the sound**, **On the screen (speed
  adjusted)**. **Interpolation** forces blending in every case.
- **Audio output**: sound card, HDMI of the output screen, Dante interface… and its volume.
- **Test patterns**: SMPTE bars, moving pattern, grid, white, grey.
- **4-corner mapping**: move the picture's corners to fit a surface; arrows = 1 pixel,
  Shift + arrows = 10 pixels on the chosen handle. While the tab is open, the output shows the
  numbered handles, the picture outline and, as an option, a grid that follows the warp; all of
  it goes away when you leave the tab. **Points intermédiaires** adds a point in the middle of
  each edge and in the centre (3 × 3 grid) to bend the picture, like Resolume's or MadMapper's
  mesh warp; moving a corner takes its points along. **Bord adouci** (soft edge) feathers the
  outline over a few pixels (3 by default): no more jaggies on slanted edges. The picture can
  never flip or cross over: a move that would fold it is refused.

Changes that need the output to restart say so: the screen goes black for a moment.

## Projects and saving

- The session (lists, settings, text) is kept continuously and restored at startup.
- **Ctrl+S** saves the project, **Ctrl+Shift+S** saves it elsewhere, **Ctrl+O** opens one,
  **Ctrl+N** starts a blank project (after confirmation).
- If media were moved, a banner says so: **Relocaliser…** looks for them by name in a folder.
- Machine preferences (output screen, decoding, sound, remotes) do not travel with the project.

## OSC control

CueStar can be driven remotely over OSC (QLab, TouchOSC, Chataigne, Companion, a script…), with
the same settings as JingleStar.

### Enable

In **Télécommandes**, **OSC** tab, tick **Enable**, set the options, then click **Apply**:

| Setting | Default | Role |
|---|---|---|
| Access | Local | **Local**: from this PC only; **Network**: from other machines |
| Port (UDP) | 9000 | Port CueStar receives commands on |
| Prefix | `/cuestar` | Start of every address |
| OSCQuery | off, port 9001 | HTTP server describing the addresses and the current state |
| Discovery | off | mDNS / Bonjour announcement for automatic discovery |
| Advertised name | CueStar | Name shown on the network |

Once applied, the window shows the IP address and listening ports, or the error if a port is
taken, then the last message received.

### Addresses

Numbers start at 1. Addresses are given with the default prefix; the original Diaporama
addresses (`/diaporama/…`) are accepted too, and so are the former Player's
(`/cuestar/lecteur/…`), served by the Cuelist.

| Address | Argument | Effect |
|---|---|---|
| `/cuestar/noir` | — | Output to black |
| `/cuestar/output/drops/reset` | — | Resets the dropped and delayed frame counter |
| `/cuestar/cuelist/goto` | int | Plays cue number n at once |
| `/cuestar/cuelist/loop` | bool | Loop the running order |
| `/cuestar/cuelist/transition` | `cut`, `black` or `fade`, then float (s) | Standby cue transition and duration |
| `/cuestar/cuelist/go` | — | Plays the standby cue (or resumes) |
| `/cuestar/cuelist/play`, `pause`, `toggle`, `stop` | — | Cuelist transport |
| `/cuestar/cuelist/next`, `prev` | — | Next / previous standby cue |
| `/cuestar/cuelist/standby` | int | Puts cue number n on standby |
| `/cuestar/diaporama/play`, `pause`, `toggle`, `stop` | — | Slideshow transport |
| `/cuestar/diaporama/next`, `prev` | — | Next / previous media |
| `/cuestar/diaporama/goto` | int | Media number n |
| `/cuestar/diaporama/settings/shuffle`, `loop`, `recursive`, `video_sound` | bool | Slideshow settings |
| `/cuestar/diaporama/settings/photo_duration` | float (1 to 60 s) | Photo duration |
| `/cuestar/diaporama/settings/fit` | `contain`, `cover` or `auto` | Fit |
| `/cuestar/diaporama/settings/folder` | string | Folder to play |

- Triggers fire with no argument or a non-zero value: a button sending 1 on press and 0 on
  release fires only once.
- Booleans accept `T`/`F`, an int or a float (≥ 0.5 = true), or `true`/`false`/`on`/`off`.
- Bundles are accepted.

### State feedback

`/cuestar/subscribe` (argument: reply port, optional) subscribes the sender for 5 s: renew it
every 2 to 3 s. CueStar sends it `media`, `paused`, `time`, `remaining`, `lecteur/playing`,
`cuelist/standby`, `cuelist/standby_name` and `diaporama/index`.

### OSCQuery

- `http://<ip>:9001/` returns the address tree as JSON (types, access, ranges and current
  values).
- `http://<ip>:9001/?HOST_INFO` returns the server information.
- `http://<ip>:9001/<path>?VALUE` returns one node's value.

Read-only nodes also give the media on screen, the position, the remaining time, the number of
items and cues, the playing item and the standby cue's name.

### Example

```
oscsend localhost 9000 /cuestar/cuelist/goto i 3
oscsend localhost 9000 /cuestar/cuelist/go
oscsend localhost 9000 /cuestar/cuelist/transition sf black 1.5
curl http://localhost:9001/cuestar/cuelist/standby_name?VALUE
```

## MIDI and console control

**Télécommandes** links MIDI controllers, a Yamaha console or a Midas M32 / Behringer X32 to
CueStar's actions, as in JingleStar:

1. Connect the source in the **MIDI**, **Yamaha** or **Midas M32 / X32** tab.
2. In **Mappings**, add a row, pick the action, click **Learn**, then touch the note, the
   controller or the console key.

Available actions: GO, next cue, previous cue, pause, stop, play cue number (from the value
received) and loop the running order for the Cuelist; play, pause, stop, next, previous for the
Slideshow; **Black**; dropped frame counter reset. Saved mappings of the former Player act on
the Cuelist.

## Keyboard and mouse shortcuts

The key list is also in **Télécommandes → Shortcuts**. Single keys do nothing while you type
in a text field.

| Action | Shortcut |
|---|---|
| Black: cut the output | **Esc** |
| Open or close help | **F1** |
| Cuelist GO; Slideshow play / pause | **Space** |
| Cuelist pause / resume | **P** |
| Previous / next standby item or cue | **Up / Down arrow** |
| Slideshow previous / next media | **Left / Right arrow** |
| Save / Save as | **Ctrl+S** / **Ctrl+Shift+S** |
| Open a project | **Ctrl+O** |
| New project | **Ctrl+N** |
| Text: bold / italic / underline | **Ctrl+B** / **Ctrl+I** / **Ctrl+U** |
| Edit an item, file information | Right-click the item |
| Play an item or a cue | Double-click |
| Move around in the preview | Click or drag on the bar under the picture |
| Search the help | Ctrl+F in the help window |

## Troubleshooting

- **The output shows a test pattern instead of the media**: libmpv was not found. Put
  `libmpv-2.dll` next to CueStar or in its `mpv` folder, then restart.
- **Output stopped** (red light): **Relancer la sortie** in the bottom bar.
- **Dropped or delayed frames keep rising**: try **GPU** decoding, **light rendering** or
  **Match the screen refresh rate to the video** in **Préférences**; **File information**
  shows whether the media is very heavy (bit rate, resolution, 10-bit).
- **No sound**: pick the card in **Préférences → Audio**.
- **Media not found**: **Relocaliser…** in the banner.
- **Odd behaviour or a crash**: the `cuestar.log` journal, in the configuration folder, keeps
  errors and crash traces.
- **OSC does not respond**: check that control is enabled and applied, the access (Local or
  Network), the port, the address prefix, and that the firewall lets the UDP port through.
