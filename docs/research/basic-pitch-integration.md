# Basic Pitch audio-to-MIDI

Fonicca runs Spotify Basic Pitch locally through ONNX Runtime. The model is
230,444 bytes (225 KiB), downloaded from Spotify's GitHub repository at commit
`fa5997af0a8210982619003269994a1be25eddf3`. SHA-256 and URL are pinned in
`src-tauri/src/stems/models.rs`. Apache-2.0 license and provenance accompany
the downloaded model. Settings support downloading and removing the pack.

The library context menu exports the source file. The editor MIDI button exports
the selection, or its entire current buffer, including unsaved edits. A native
save dialog chooses the destination. Work runs in the background with progress,
cancellation, model-use locking and atomic MIDI writes. Original audio remains
unchanged. Inputs are mono/stereo, limited to ten minutes.

The first version exports polyphonic pitches, timings and velocities, without
pitch bends. It is most useful for isolated bass, piano or guitar recordings.
SMF format 0 uses one track, 480 ticks per quarter, and the known BPM or 120.
Changing the tempo grid preserves note timing in seconds. Python is not required
by the application.

## Reference verification

The Rust implementation follows Spotify's 22,050 Hz mono preprocessing,
43,844-sample windows, 36,164-sample stride, 15-frame trimming and time-alignment
correction. It uses default onset/frame thresholds of 0.5/0.3, minimum note
length 11 frames and the reference polyphonic decoder's energy tolerance.
A heap replaces repeated whole-matrix maximum searches while preserving ties.

`basic_pitch::tests::real_model_inference` (ignored by default) accepts
`BASIC_PITCH_MODELS`, `BASIC_PITCH_AUDIO` and optional `BASIC_PITCH_OUTPUT` to
write diagnostic activations, notes, input WAV and MIDI. Run
`verify-basic-pitch.py OUTPUT MODEL` with the pinned Spotify package on
`PYTHONPATH` and its scientific Python dependencies installed separately.

On M1 Pro, a 6.803-second bass stem produced 30 notes. Native inference and note
decoding took 217 ms in one development-profile measurement, excluding
file decoding/resampling. This observation is not a general performance
guarantee. Independent reference inference differed by less than 3e-8 in
frame/onset activations. With identical activations, all pitches, velocities and
start/end times matched Spotify's Python decoder. Reading the MIDI back with
PrettyMIDI preserved pitches/velocities and timings within one MIDI tick.

Validation covers silence, short notes, polyphony/repeated notes, cancellation,
invalid input, MIDI event ordering, and preserving existing files on errors or
cancellation. A real network test downloads, verifies, deletes and redownloads
the model while preserving an unrelated WAV. UI checks in the in-app browser
cover whole edited audio, selected audio, completion and the library context
menu; browser data is mocked, inference checks are native.
