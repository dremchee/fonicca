import { onScopeDispose, shallowRef } from 'vue'
import type { ModelStorage } from '@/api'
import { api } from '@/api'
import { createSharedJobPoller } from '@/job-poller'

const models = shallowRef<ModelStorage[] | null>(null)
const error = shallowRef('')
const status = createSharedJobPoller({
  read: api.modelStorage, intervalMs: 1000, running: () => true,
  apply(next) { models.value = next; error.value = '' },
  onError(cause) { error.value = String(cause) },
})

/** Download badges and visible settings share the same model-storage requests. */
export function useModelStorage() {
  const lease = status.acquire()
  onScopeDispose(lease.release)
  return { models, error, refresh: lease.refresh }
}
