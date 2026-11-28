# Apply the same processing to multiple files {#overview}

Use batch processing to prepare a folder of samples with consistent trimming, fades, levels or WAV settings. Each input file can be up to 30 minutes long.

## Build a recipe {#recipe}

[Select the rows](library.md#navigate), right-click a selected row and choose **Processing → Process…**. The dialog shows how many files are selected. Enable the steps you need.

| Step | How to use it |
| --- | --- |
| **Trim silence** | Set **Silence below** in dB and **Keep around the sound** in milliseconds. Allow padding when you want to retain room tone or part of a decay. |
| **Remove DC offset** | Center audio around zero before leveling and fades. |
| **Normalize → Peak** | Set a maximum sample level in dBFS. Peak normalization is applied before fades and sample-rate conversion. |
| **Normalize → LUFS** | Set perceived loudness and a sample peak ceiling. LUFS runs after fades and conversion and needs at least 400 ms of audible audio. The report identifies files limited by the ceiling. |
| **Fades** | Set fade-in and fade-out lengths in milliseconds and choose the curve. |
| **Format** | Set channels, sample rate and bit depth, or keep the existing values where supported. Batch output is always WAV. |
| **Rename** | Use a template for output files. For example, `{n}_{name}` numbers each clip. You can also insert `{category}`, `{bpm}`, `{key}` or `{kind}` where those values are known. |

## Save copies or replace WAVs {#destination}

Choose **A folder** to write new WAVs and preserve the source files. For example, write trimmed drum hits to a dedicated folder, then check the first few for clipped attacks or lost tails. If needed, rerun with more padding.

**Replace the originals** is for WAV inputs. They keep their names, and Fonicca copies each original to its backups folder before replacing it. For mixed input formats, use a separate output folder.

## Run and review {#results}

1. Click **Process**. Watch progress in the dialog and review the number of files written and the list of failures. If you stop a batch, check the summary for files already written.
2. Check the results, especially output names and the loudness report for files limited by the peak ceiling.
3. Use **Save as preset…** to store a recipe. Pick it from **Preset** next time and verify the destination for the new batch.
