import type { StemMode, StemModel } from './api'
import { modelCatalog, stemModels } from './model-catalog.generated.ts'
export { stemModels }
export const stemModelBytes = Object.fromEntries(stemModels.map(model => [model, modelCatalog[model].bytes])) as Record<StemModel, number>
export function isStemModel(value: unknown): value is StemModel { return stemModels.includes(value as StemModel) }
export function modelsForMode(mode: StemMode): StemModel[] {
  return stemModels.filter(model => modelCatalog[model].tracks === (mode === 'two' ? 2 : 4))
}
export function stemNames(mode: StemMode) { return mode === 'two' ? ['vocals', 'instrumental'] as const : ['vocals', 'drums', 'bass', 'other'] as const }
