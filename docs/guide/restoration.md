# Restore compressed music with Apollo {#overview}

Apollo processes music that has gone through lossy compression. Compare the result by ear before using it in a project. Fonicca saves the result in a new file, so you can keep both versions.

## Restore a track {#restore}

1. In the library, right-click a mono or stereo track up to 10 minutes long. Choose **Processing → Restore with Apollo…**.
2. Click **Restore**, or **Download and restore** if the model is missing. The model download needs internet access. Processing runs in the background on your computer.
3. When it finishes, audition **Original** and **After Apollo** from the result dialog. Use **Open in editor** to inspect or edit the restored version.

The restored copy is a 44.1 kHz, 32-bit float WAV marked **Apollo** beside the source. Fonicca adds the new file to the library. The original audio stays in place.

## If restoration fails {#troubleshooting}

Check that the source is mono or stereo, that its folder allows writing and that the model is installed under **Settings → Analysis → Models**. For a long recording, [save a selection](export.md#export-selection) and restore that excerpt. **Stop** requests cancellation after the current fragment.
