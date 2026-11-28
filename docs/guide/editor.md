# Edit and process audio {#overview}

Open the editor by double-clicking a track or choosing **Edit…** in its context menu. The editor opens below the library. **Open editor in a separate window** moves the current edit into its own window, and **Return editor to the main window** brings it back.

## Select and listen {#selection}

- Click the waveform to place the cursor. Drag over the audio to select a range. Use **Esc** to clear the selection.
- Press **Space** with the editor focused to listen to the selection or play from the cursor. Use **Loop** to repeat a selected passage while you adjust it.
- Use the overview of the whole file to navigate. Drag the middle of its highlighted range to pan, drag its edges to change the range, or double-click the overview to fit the file.
- Choose **Spectrogram** under **Audio display** to see where frequencies occur over time. You can select audio in either display.

## Line up an edit precisely {#grid-and-zoom}

Open **View → Grid** and select **Bars and beats**, **Time** or **Off**. For rhythmic editing, set **Grid tempo** to match the recording. Place the cursor on the first downbeat and choose **Bar 1 starts at the cursor** to align the grid with the music. Use **Snap to grid** for regular boundaries and **Snap to zero crossings** to help place cuts near zero amplitude.

## Cut, paste and fade {#editing-tools}

| Task | Tool and effect |
| --- | --- |
| Keep only the chosen excerpt | Select it and choose **Crop**. The rest is removed from the current edit. |
| Remove part of the audio and close the gap | Select it and choose **Delete**. **Cut** also puts the audio on the editor clipboard. |
| Keep the timing while removing the sound | Select a passage and choose **Silence**. |
| Reuse a passage | **Copy**, then place the cursor and **Paste**. With a selection, paste can replace the selected range. You can paste audio into another Fonicca editor window. |
| Smooth an edge | Select the short part you want to fade and use **Fade in** or **Fade out**. Choose a curve to control the transition. With no selection, the cursor marks the end of a fade-in or the start of a fade-out. |

For example, to clean up a drum hit, zoom in, crop to the attack and tail, then select just the last part of the tail and apply **Fade out**. Audition it before saving.

## Change level, pitch or duration {#processing}

For these tools, select a passage to process it alone or clear the selection to work on the whole file. Some commands are in **More tools**, depending on the available toolbar space.

- **Normalize** sets the peak to −0.3 dBFS. Use the gain controls for smaller changes in 3 dB steps.
- **Normalize loudness (LUFS)** adjusts perceived loudness. Enter a target and a sample peak ceiling. The tool needs at least 400 ms of audible audio and can reduce the gain to respect the ceiling. The result shows the measured loudness before and after.
- **Pitch / time-stretch** changes pitch and duration independently. Pitch accepts −24 to +24 semitones. Duration accepts 25% to 400%, with 100% preserving length. For example, set pitch to 0 and duration to 200% to double the length at the original pitch.
- **Remove DC offset** centers each channel around zero. **Reverse** plays the selected audio, or the whole file, in reverse order once applied.

Use **Undo** or **Redo** to compare your edits. When closing the editor or opening another track, Fonicca prompts about unsaved edits. Use [Save as or Save selection](export.md) to write audio to disk and [Loop points or Slice](loops-slices.md) to mark recurring passages or cuts.
