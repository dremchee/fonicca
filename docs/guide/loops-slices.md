# Create loops and slice recordings {#overview}

A loop repeats one range. Slice markers divide a file into regions you can audition or export separately.

## Audition and save a loop {#loop-points}

1. Open a track in the editor and select the passage. Use the [grid and zoom tools](editor.md#grid-and-zoom) to check both boundaries.
2. Turn on **Loop** and start editor playback. Listen across the join and adjust the selection.
3. Use **Loop points** or **Shift + L** to set the persistent boundaries from the selection. You can drag the loop flags on the ruler to refine them. To clear the points, clear the selection and use the same command.
4. [Save as WAV](export.md#formats) to include these points in the audio file. Whether a DAW or sampler uses them depends on its WAV metadata support.

Turning on **Loop** controls auditioning in the editor. Use **Loop points** to write the boundaries on export.

## Place markers by hand {#manual-slices}

Double-click the ruler or press **M** at the cursor to add a marker. Press **M** with a selection to mark both edges. Drag a marker flag to move the boundary. Its context menu offers removal. Click a region on the ruler to select it, or use **1–9** and **Shift + 1–9** to audition regions 1–18.

## Detect cuts automatically {#automatic-slices}

1. Select a range to work on that part of the recording, or clear the selection to cover the whole file. Open **Slice**.
2. Pick the mode that fits the material.

   | Mode | Controls |
   | --- | --- |
   | **Grid** | Set a BPM in the editor and pick a beat or bar division. Useful for splitting a loop into equal steps. |
   | **Hits** | Adjust sensitivity and the shortest allowed slice. Useful for separating drum hits by their attacks. |
   | **Silence** | Adjust the level below which audio counts as silence and the shortest gap. Useful for separating sounds recorded with pauses. |

3. Review the detected regions and apply the markers. Refine them by dragging the flags and audition each region before exporting.

## Save individual regions {#save-regions}

Hold **Option / Alt** while clicking a region on the ruler to skip it. Use the same gesture or the region menu to include it again. Choose **Save regions** in the save menu to write the included audio as numbered WAV files in a new slices folder beside the current track. The editor stays open on the full track. If the output folder is outside the indexed library, use the file manager to open the exported files.
