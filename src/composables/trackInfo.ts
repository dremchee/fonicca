import type { Sample } from '@/api'
import { ref, shallowRef } from 'vue'

/** The sample whose track info dialog is open; rendered by TrackInfoDialog in App. */
export const trackInfoFor = shallowRef<Sample | null>(null)

/** Bumped after tags are saved, so views showing them read them again. */
export const trackInfoRevision = ref(0)

export function openTrackInfo(sample: Sample | null | undefined) {
  if (sample)
    trackInfoFor.value = sample
}
