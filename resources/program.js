const {
  print_memory,
  bytesOf,
  REMOVE_UNNECESSARY,
  USE_CACHE,
  COPY_ON_APPLICATION,
  FORMATION,
  GC_ENABLED_DEFAULT,
  APPLICATION,
  DISPATCH,
  COPY,
  SET,
  memory,
  PHI,
  DELTA,
  RHO,
  LAMBDA
} = require('./helpers.js');

let program_size = 0, peak_live = 0, total_created = 0, total_deleted = 0, max_ref_holders = 0, live_count = 0

const push = (obj) => {
  memory.push(obj)
  ++total_created
  if (++live_count > peak_live) peak_live = live_count
}

const trim = () => {
  while (memory.length > 0 && memory[memory.length - 1] == null) memory.length--
}

const del = (idx) => {
  memory[idx] = null
  ++total_deleted
  --live_count
  ref_holders.delete(idx)
}

const pop = () => {
  del(head())
  memory.length--
}

const head = () => {
  if (memory.length === 0) throw new Error("Can't get head from empty memory")
  return memory.length - 1
}

let phi_watermark = -1

let ref_holders = new Set()

const remap_refs = (obj, r) => {
  Object.keys(obj.target).forEach((at) => {
    const atr = obj.target[at]
    if (atr.cache != null) {
      const nc = r(atr.cache)
      if (nc !== atr.cache) { atr.cache = nc; obj.written_attrs.add(at) }
    } else if (atr.xi != null) {
      const nv = r(atr.value), nx = r(atr.xi)
      if (nv !== atr.value || nx !== atr.xi) { atr.value = nv; atr.xi = nx; obj.written_attrs.add(at) }
    } else if (atr.value != null) {
      const nv = r(atr.value)
      if (nv !== atr.value) { atr.value = nv; obj.written_attrs.add(at) }
    }
  })
}

const compact = (from, to, pivot) => {
  const end = Math.min(to, memory.length - 1)

  let cursor = from
  while (cursor <= end && memory[cursor] != null && memory[cursor].stay) {
    memory[cursor].stay = null
    ++cursor
  }
  if (cursor > end) return pivot

  const first_dest = cursor
  let dest = cursor
  for (let i = cursor; i <= end; ++i) {
    const obj = memory[i]
    if (obj == null) continue
    if (obj.stay) {
      obj.stay = null
      obj.fwd = dest
      ++dest
    } else {
      del(i)
    }
  }

  const r = (idx) => {
    if (idx < first_dest || idx > end) return idx
    const obj = memory[idx]
    return (obj != null && obj.fwd != null) ? obj.fwd : idx
  }

  for (let i = from; i <= end; ++i) {
    if (memory[i] != null) remap_refs(memory[i], r)
  }

  const next_holders = new Set()
  for (const idx of ref_holders) {
    const cur = r(idx)
    const obj = memory[idx]
    if (obj == null) continue
    next_holders.add(cur)
    if (idx < from || idx > end) {
      for (const at of obj.written_attrs) {
        const atr = obj.target[at]
        if (atr == null) continue
        if (atr.cache != null) atr.cache = r(atr.cache)
        else if (atr.xi != null) { atr.value = r(atr.value); atr.xi = r(atr.xi) }
        else if (atr.value != null) atr.value = r(atr.value)
      }
    }
  }

  ref_holders = next_holders
  if (ref_holders.size > max_ref_holders) max_ref_holders = ref_holders.size
  if (phi_watermark >= 0) phi_watermark = r(phi_watermark)
  const result = r(pivot)

  for (let i = first_dest; i <= end; ++i) {
    const obj = memory[i]
    if (obj == null || obj.fwd == null) continue
    const dst = obj.fwd
    memory[dst] = obj
    if (dst !== i) memory[i] = null
  }
  trim()

  return result
}

const gc_phi = (gc_enabled, value, scope) => {
  gc_enabled = gc_enabled && (phi_watermark < 0 || value > phi_watermark)
  if (gc_enabled) {
    phi_watermark = value
    if (value - scope > 1) {
      mark_phi(value, scope)
      value = compact(scope + 1, value, value)
    }
  }
  return value
}

const gc_disp = (from, phi) => {
  mark_disp(from, from, phi)
  mark_disp(phi, from, phi)
  return compact(from, phi, phi)
}

const attr_ref = (atr) => {
  if (atr.cache == null && atr.xi != null) return atr.xi
  if (atr.cache != null) return atr.cache
  return null
}

const mark = (index, in_range, recurse) => {
  memory[index].stay = true
  Object.keys(memory[index].target).forEach((at) => {
    const ref = attr_ref(memory[index].target[at])
    if (ref != null && in_range(ref) && memory[ref] != null && !memory[ref].stay) {
      recurse(ref)
    }
  })
}

const mark_disp = (start, from, to) =>
  mark(start, (ref) => ref >= from && ref <= to, (ref) => mark_disp(ref, from, to))

const mark_phi = (index, scope) =>
  mark(index, (ref) => ref > scope && ref < phi_watermark, (ref) => mark_phi(ref, scope))

const attr = (value, xi = null, cache = null) => ({value, xi, cache})

// objects
const object = (name, type, target, attr = null, value = null) => ({
  name,
  type,
  target,
  attr,
  value,
  written_attrs: new Set()
})

const formation = (name, attrs) => object(name, FORMATION, attrs)

const dispatch = (name, target, attr) => object(name, DISPATCH, target, attr)

const application = (name, target, attr, value) => object(name, APPLICATION, target, attr, value)

// OPERATIONS
const operation = (type, target, attr = null, value = null) => ({type, target, attr, value})

const copy = (target) => operation(COPY, target)

const set = (target, attr, value) => operation(SET, target, attr, value)

const atoms = {
  'L_number_plus': (self) => {
    push(dispatch(`${self}.${RHO}`, self, RHO))
    const left = bytesOf.bytes(dataize(morph(head(), self, true))).asNumber()

    push(dispatch(`${self}.x`, self, 'x'))
    const right = bytesOf.bytes(dataize(morph(head(), self, true))).asNumber()

    push(dispatch('Q.number', 0, 'number'))
    const num = morph(head(), self, true)

    push(dispatch('Q.bytes', 0, 'bytes'))
    const bts = morph(head(), self, true)

    push(formation('plus res', {[DELTA]: attr(bytesOf.number(left + right).asBytes())}))
    const data = morph(head(), self, true)

    push(application(`${bts}(0: ${data})`, bts, 0, data))
    const _bts = morph(head(), self, true)

    push(application(`${num}(0: ${_bts})`, num, 0, _bts))
    const res = morph(head(), self, true)

    memory[res].from_atom = true

    return res
  },
  'L_number_gt': (self) => {
    push(dispatch(`${self}.${RHO}`, self, RHO))
    const left = bytesOf.bytes(dataize(morph(head(), self, true))).asNumber()

    push(dispatch(`${self}.x`, self, 'x'))
    const right = bytesOf.bytes(dataize(morph(head(), self, true))).asNumber()

    const bool = left > right ? 'true' : 'false'

    push(dispatch('Q.' + bool, 0, bool))
    const res = morph(head(), self, true)

    memory[res].from_atom = true

    return res
  },
  'L_number_times': (self) => {
    push(dispatch(`${self}.${RHO}`, self, RHO))
    const left = bytesOf.bytes(dataize(morph(head(), self, true))).asNumber()

    push(dispatch(`${self}.x`, self, 'x'))
    const right = bytesOf.bytes(dataize(morph(head(), self, true))).asNumber()

    push(dispatch('Q.number', 0, 'number'))
    const num = morph(head(), self, true)

    push(dispatch('Q.bytes', 0, 'bytes'))
    const bts = morph(head(), self, true)

    push(formation('times res', {[DELTA]: attr(bytesOf.number(left * right).asBytes())}))
    const data = morph(head(), self, true)

    push(application(`${bts}(0: ${data})`, bts, 0, data))
    const _bts = morph(head(), self, true)

    push(application(`${num}(0: ${_bts})`, num, 0, _bts))
    const res = morph(head(), self, true)

    memory[res].from_atom = true

    return res
  },
}

const exec = (op) => {
  let res, tgt
  switch (op.type) {
    case COPY:
      const clone = structuredClone(memory[op.target])
      push({...clone, name: '(copy ' + clone.name + ')'})
      res = head()
      for (const at of Object.keys(memory[res].target)) memory[res].written_attrs.add(at)
      ref_holders.add(res)
      if (ref_holders.size > max_ref_holders) max_ref_holders = ref_holders.size
      break
    case SET:
      tgt = memory[op.target].target

      let at
      if (typeof op.attr === 'string') {
        at = op.attr
      } else {
        at = Object.keys(tgt)[op.attr]
      }

      tgt[at] = op.value
      memory[op.target].written_attrs.add(at)
      res = op.target
      ref_holders.add(op.target)
      if (ref_holders.size > max_ref_holders) max_ref_holders = ref_holders.size
      break
  }
  return res
}

const needs_context = (index) => {
  const obj = memory[index]
  let need
  switch (obj.type) {
    case FORMATION:
      need = false
      break
    case DISPATCH:
      need = obj.target === -1 || needs_context(obj.target)
      break
    case APPLICATION:
      need = obj.target === -1 || obj.value === -1 || needs_context(obj.target) || needs_context(obj.value)
      break
  }
  return need
}

const morph = (index, context, remove) => {
  const obj = memory[index]
  const clear = REMOVE_UNNECESSARY && remove
  let res, tgt_i, at
  switch (obj.type) {
    case FORMATION:
      res = index
      break
    case DISPATCH:
      if (clear) {
        pop()
      }

      if (obj.target === -1) {
        tgt_i = context
      } else {
        tgt_i = morph(obj.target, context)
      }
      const tgt = memory[tgt_i].target

      if (obj.attr === '-1') {
        res = context
      } else {
        if (Object.hasOwn(tgt, obj.attr)) {
          let at_i
          if (USE_CACHE && tgt[obj.attr].cache != null) {
            at_i = tgt[obj.attr].cache
          } else {
            at = tgt[obj.attr]

            let ctx
            if (at.xi != null) {
              ctx = at.xi
            } else {
              ctx = tgt_i
            }
            at_i = morph(at.value, ctx)

            if (USE_CACHE && tgt[obj.attr].cache == null) {
              tgt[obj.attr].cache = at_i
              memory[tgt_i].written_attrs.add(obj.attr)
              ref_holders.add(tgt_i)
              if (ref_holders.size > max_ref_holders) max_ref_holders = ref_holders.size
            }
          }

          if (at_i !== 0 && obj.attr !== RHO && !Object.hasOwn(memory[at_i].target, RHO)) {
            res = exec(copy(at_i))
            if (USE_CACHE) {
              res = exec(set(res, RHO, attr(tgt_i, null, tgt_i)))
            } else {
              res = exec(set(res, RHO, attr(tgt_i)))
            }
          } else {
            res = at_i
          }
        } else if (Object.hasOwn(tgt, PHI)) {
          push(dispatch(`${tgt_i}.${PHI}`, tgt_i, PHI))
          let phi_i = morph(head(), tgt_i, true)
          phi_i = gc_disp(tgt_i, phi_i)
          push(dispatch(`${phi_i}.${obj.attr}`, phi_i, obj.attr))
          res = morph(head(), phi_i, true)
          res = gc_disp(tgt_i, res)
        } else if (Object.hasOwn(tgt, LAMBDA)) {
          const atom = tgt[LAMBDA].value
          if (!Object.hasOwn(atoms, atom)) {
            throw new Error(`Atom ${atom} does not exist`)
          }
          const atom_res_i = morph(atoms[atom](tgt_i), tgt_i)
          push(dispatch(`${atom_res_i}.${obj.attr}`, atom_res_i, obj.attr))
          res = morph(head(), atom_res_i, true)
        } else {
          throw new Error(`Bad dispatch on ${index}, can't go though ${obj.attr}, ${PHI} or ${LAMBDA}`)
        }
      }
      break
    case APPLICATION:
      if (clear) {
        pop()
      }

      tgt_i = morph(obj.target, context)

      if (obj.value === -1) {
        at = attr(context)
      } else if (needs_context(obj.value)) {
        at = attr(obj.value, context)
      } else {
        at = attr(obj.value)
      }

      if (COPY_ON_APPLICATION) {
        tgt_i = exec(copy(tgt_i))
      }

      if (USE_CACHE && memory[at.value].type === FORMATION) {
        at.cache = at.value
      }

      res = exec(set(tgt_i, obj.attr, at))
      break
  }
  return res
}

const dataize = (index, scope = program_size - 1, gc_enabled = GC_ENABLED_DEFAULT) => {
  const obj = memory[index]
  let data
  switch (obj.type) {
    case FORMATION:
      if (Object.hasOwn(obj.target, DELTA)) {
        data = obj.target[DELTA].value
      } else if (Object.hasOwn(obj.target, PHI)) {
        push(dispatch(`${obj.name}.${PHI}`, index, PHI))
        let phi_i = morph(head(), index, true)
        phi_i = gc_phi(gc_enabled, phi_i, scope)
        data = dataize(phi_i, scope, gc_enabled)
      } else if (Object.hasOwn(obj.target, LAMBDA)) {
        const atom = obj.target[LAMBDA].value
        if (!Object.hasOwn(atoms, atom)) {
          throw new Error(`Atom ${atom} does not exist`)
        }
        let atom_res_i = morph(atoms[atom](index), index)
        atom_res_i = gc_phi(gc_enabled, atom_res_i, scope)
        data = dataize(atom_res_i, scope, gc_enabled)
      } else {
        throw new Error(`Can't dataize object ${index}, no ${DELTA}, no ${PHI}, no ${LAMBDA}`)
      }
      break
    default:
      const op_i = morph(index, index, true)
      data = dataize(op_i, scope, gc_enabled)
      break
  }
  return data
}

// OBJECTS

program_size = memory.length
live_count = memory.length
peak_live = memory.length
total_created = memory.length

try {
  const res = bytesOf.bytes(dataize(0, head(), true)).asNumber()
  print_memory()
  console.log(`data: ${res}`)
  console.log(`original program size: ${program_size}`)
  console.log(`total created: ${total_created}`)
  console.log(`total created without program: ${total_created - program_size}`)
  console.log(`total deleted: ${total_deleted}`)
  console.log(`max depth: ${peak_live}`)
  console.log(`max depth without program: ${peak_live - program_size}`)
  console.log(`max ref holders: ${max_ref_holders}`)
} catch (e) {
  console.log(e)
  print_memory()
  throw e
}