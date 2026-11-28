# Audition samples and send them to a DAW {#overview}

## Start and control a preview {#mini-player}

Click a row to hear the sample in the floating mini-player. Use its transport to play, pause, stop, or navigate to the previous or next track. Click or drag the progress bar to seek. Drag the body of the mini-player to move it. When the panel itself has focus, arrow keys reposition it and **Home** returns it to the bottom.

To audition several sounds quickly, use the library's up and down arrows. **Settings → Audio → Autoplay** determines whether moving focus starts playback. With the library focused, **Space** toggles the focused track and **Esc** stops the preview. The editor has its own playback controls.

## Choose how a row starts playing {#click-behavior}

Open **Settings → Audio → Clicking a track**.

| Mode | What a click does |
| --- | --- |
| **From the click position** | Starts or seeks where you click on a loaded waveform. |
| **Always from the beginning** | Starts from 0:00 on each click. |
| **New track from the beginning** | Starts a newly selected track at 0:00. Click its waveform again to seek. |
| **Option / Alt + click to seek** | Starts at 0:00 with a plain click. Hold **Option / Alt** while clicking a loaded waveform to play from that point. |

If the waveform is still loading, use the mini-player progress bar to seek. Open [Track info](track-info.md) to see sample properties and available analysis details.

## Set the output and level {#audio-output}

Open **Settings → Audio → Output device** and choose a system output or audio interface. Changing the device stops the current playback, so start it again to test the new output. If the device is missing after reconnecting it, choose **Refresh devices**. Check the mini-player's mute control and preview volume if playback appears to be running silently.

## Drag a file into your DAW {#daw}

1. Open the target project in a DAW that accepts file drops.
2. Drag the sample row into the target track or sampler. To transfer several files, [select the rows](library.md#navigate) first and drag from one of them.
3. In the DAW, check the imported clips and adjust their levels there.

The drop uses the existing audio files at their saved levels. Preview volume affects listening in Fonicca. To send processed audio, [export the edited file](export.md) first and drag that file instead.
