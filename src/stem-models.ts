import type { StemMode, StemModel } from './api'
export const stemModelBytes: Record<StemModel, number> = {
  spleeter: 78856560, scnetLarge: 45082486, scnetXlIhf: 221482626, htdemucs: 316446953,
  melRoformer: 464050097, bsRoformer: 644830379, mdx23c: 448322798,
}
export const stemModels: StemModel[] = ['scnetXlIhf', 'scnetLarge', 'htdemucs', 'spleeter', 'melRoformer', 'bsRoformer', 'mdx23c']
export function isStemModel(value: unknown): value is StemModel { return stemModels.includes(value as StemModel) }
export function modelsForMode(mode: StemMode): StemModel[] {
  return mode === 'two' ? ['melRoformer', 'bsRoformer', 'mdx23c'] : ['scnetXlIhf', 'scnetLarge', 'htdemucs', 'spleeter']
}
export function stemNames(mode: StemMode) { return mode === 'two' ? ['vocals', 'instrumental'] as const : ['vocals', 'drums', 'bass', 'other'] as const }
