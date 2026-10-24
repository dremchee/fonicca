// Fake duplicate groups shaped like the real library: the same kits redistributed
// in several packs inside one source folder.

export interface Copy {
  id: number
  /** Relative to the source root. */
  path: string
  /** Days since modification, for the oldest/newest rules. */
  age: number
}

export interface Group {
  id: number
  size: number
  duration: number
  /** Large files whose fingerprint still waits for a full-content check. */
  verifying: boolean
  copies: Copy[]
}

const PACKS = [
  ['+ Producers Kits 1_2/9th Wonder Kit', 'Drum Kits from Dj Beatstone/01. DEADLY DRUMS/Deadly Drums 1'],
  ['Splice/Boom Bap Essentials', 'Downloads/BoomBap_Essentials_v2', 'Old Drive Backup/Samples/BoomBap Essentials'],
  ['Kits/Lo-Fi Dust Vol 3', 'Kits/Lo-Fi Dust Vol 3 (copy)'],
  ['Vengeance/Essential House Vol 2', 'Torrents/VEH2/Vengeance Essential House 2'],
]

const SOUNDS = [
  ['Cymbals/Str_Oh1.wav', 'Cymbals/Str_Oh1.wav', 2_032_472, 5.8],
  ['Percussions/Str_Chim.wav', 'Percusn/Str_Chim.wav', 2_029_788, 5.7],
  ['Kicks/BB_Kick_Dusty_01.wav', 'kicks/bb kick dusty 01.wav', 98_340, 0.5],
  ['Snares/BB_Snare_Crack.wav', 'Snares/BB_Snare_Crack.wav', 120_118, 0.6],
  ['Hats/LoFi_Hat_Closed_04.wav', 'Hats/LoFi_Hat_Closed_04.wav', 44_910, 0.2],
  ['Loops/Dust_Loop_90_Am.wav', 'Loops/Dust Loop 90 Am.wav', 1_623_404, 10.7],
  ['Claps/VEH2 Clap 017.wav', 'Claps/VEH2 Clap 017.wav', 154_880, 0.8],
  ['Kicks/VEH2 Kick 101.wav', 'Kick/VEH2 Kick 101.wav', 211_200, 1.1],
  ['Toms/Str_Tom_Lo.wav', 'Toms/Str_Tom_Lo.wav', 402_118, 1.1],
  ['FX/Vinyl_Crackle_Long.wav', 'FX/Vinyl Crackle Long.wav', 5_291_600, 30],
] as const

let id = 1
export const groups: Group[] = Array.from({ length: 36 }, (_, i) => {
  const [a, b, size, duration] = SOUNDS[i % SOUNDS.length]!
  const packs = PACKS[i % PACKS.length]!
  return {
    id: i + 1,
    size: size + i * 37,
    duration,
    verifying: size > 131_072 && i % 5 === 0,
    copies: packs.map((p, k) => ({ id: id++, path: `${p}/${k === 0 ? a : b}`, age: 900 - k * 250 + i })),
  }
})
