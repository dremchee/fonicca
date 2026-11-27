import { cloneVNode, Comment, Fragment, isVNode, Text, type ComponentPublicInstance, type FunctionalComponent, type VNode } from 'vue'

type TriggerRef = (element: Element | null) => void
type VueRef = (element: Element | ComponentPublicInstance | null) => void
const refs = new WeakMap<TriggerRef, VueRef>()

function forwardRef(ref: TriggerRef): VueRef {
  const previous = refs.get(ref)
  if (previous) return previous
  const bind: VueRef = (target) => {
    const element = target instanceof Element ? target : target?.$el
    ref(element instanceof Element ? element : null)
  }
  refs.set(ref, bind)
  return bind
}

// Put the SDK attributes on the caller's trigger. Vue component refs are
// public instances, while the SDK's behavior binds to the underlying element.
function triggerNode(nodes: VNode[]): VNode | undefined {
  for (const node of nodes) {
    if (node.type === Fragment && Array.isArray(node.children)) {
      const child = triggerNode(node.children.filter(isVNode))
      if (child) return child
    }
    else if (node.type !== Comment && node.type !== Text) return node
  }
}

const RenderChild: FunctionalComponent<{ triggerProps: Record<string, unknown> }> = ({ triggerProps }, { slots }) => {
  const child = triggerNode(slots.default?.() ?? [])
  if (!child) return null
  const { ref, ...attrs } = triggerProps
  return cloneVNode(child, typeof ref === 'function' ? { ...attrs, ref: forwardRef(ref as TriggerRef) } : attrs, true)
}
RenderChild.props = ['triggerProps']

export default RenderChild
