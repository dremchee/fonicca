// Generated from model-packs/catalog.json. Run pnpm models:generate; do not edit.
// Catalog FNV-1a64: d018ee76adc4a9a9
export const modelCatalog = {
  "clap": {
    "name": "CLAP",
    "group": "analysis",
    "bytes": 34301667,
    "tracks": null
  },
  "spleeter": {
    "name": "Spleeter",
    "group": "separation",
    "bytes": 78856560,
    "tracks": 4
  },
  "scnetLarge": {
    "name": "SCNet-large",
    "group": "separation",
    "bytes": 45082486,
    "tracks": 4
  },
  "scnetXlIhf": {
    "name": "SCNet XL IHF",
    "group": "separation",
    "bytes": 221482626,
    "tracks": 4
  },
  "htdemucs": {
    "name": "HTDemucs",
    "group": "separation",
    "bytes": 316446953,
    "tracks": 4
  },
  "melRoformer": {
    "name": "Mel-Band RoFormer Kim",
    "group": "separation",
    "bytes": 464050097,
    "tracks": 2
  },
  "bsRoformer": {
    "name": "BS-RoFormer viperx",
    "group": "separation",
    "bytes": 644830379,
    "tracks": 2
  },
  "mdx23c": {
    "name": "MDX23C",
    "group": "separation",
    "bytes": 448322798,
    "tracks": 2
  },
  "apollo": {
    "name": "Apollo",
    "group": "restoration",
    "bytes": 66541845,
    "tracks": null
  },
  "basicPitch": {
    "name": "Basic Pitch",
    "group": "transcription",
    "bytes": 230444,
    "tracks": null
  }
} as const

export const stemModels = ["scnetXlIhf","scnetLarge","htdemucs","spleeter","melRoformer","bsRoformer","mdx23c"] as const
export type StemModel = typeof stemModels[number]
export type ModelId = keyof typeof modelCatalog
export type ModelGroup = typeof modelCatalog[ModelId]['group']
