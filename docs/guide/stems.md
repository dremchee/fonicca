# Separate a track into stems {#overview}

Use stem separation to get vocals and accompaniment, or extract drums or bass from a mix. Fonicca separates mono or stereo tracks up to 10 minutes. The result is separate stereo audio files at 44.1 kHz, and the source stays in place.

## Choose a mode and model {#choose-model}

1. Right-click the original track and choose **Processing → Separate into stems…**.
2. Select **2 tracks** for vocals and instrumental, or **4 tracks** for vocals, drums, bass and other instruments.
3. Choose a **Separation model**. For a quick four-part audition, try the model marked **Fast**. For a more detailed pass, use the descriptions and download sizes to compare the available options. Use a two-track model if you need a vocal over the complete backing track.
4. Check the model's [usage terms](https://github.com/dremchee/fonicca/blob/main/docs/dependencies.md) for your intended use. Model files are downloaded on first use and audio is processed locally.

## Choose where the stems go {#output}

Check **Output format** and **Output folder** before you run the job. Fonicca preselects the source format if supported, or WAV otherwise. Use WAV or FLAC for lossless storage. By default each track gets a subfolder named after it beside the original. To save new stems elsewhere, set **Settings → Analysis → Stem storage**. This changes the destination of future jobs.

## Separate and use the results {#results}

1. Click **Separate** or **Download model and separate**. Fonicca shows progress through download, separation, encoding and library import as applicable. **Stop** requests cancellation after the current step.
2. Open the original track in the library and click its numbered badge to expand the result. You can preview individual stems, open them in the editor or drag their files into a DAW.
3. Compare the result against the mix. If the source contains audible bleed between parts, you can try another model or edit the extracted passage.

If a job fails, check the track length, channel count, available space for the model and output, and write access to the destination. To work on a passage from a longer recording, [export an excerpt](export.md#export-selection) first, put it in the library and separate that excerpt.
