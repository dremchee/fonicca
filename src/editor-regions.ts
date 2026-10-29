/** Starts a region that runs to the next marker; `skip` leaves the region out when saving. */
export interface Marker { frame: number, skip: boolean }

/** A region between markers. */
export interface Region { start: number, end: number, skip: boolean }

/** Only explicit markers start regions; audio before the first marker stays unmarked. */
export function regionsOf(markers: Marker[], frames: number): Region[] {
  return markers.map((marker, index) => ({
    start: marker.frame,
    end: markers[index + 1]?.frame ?? frames,
    skip: marker.skip,
  }))
}
