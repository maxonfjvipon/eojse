const {
  print_memory,
  bytesOf,
  REMOVE_UNNECESSARY,
  USE_CACHE,
  COPY_ON_APPLICATION,
  USE_PHI_POINTS,
  REMOVE_ON_POINTS,
  FORMATION,
  WITH_PHI_POINTS_DEFAULT,
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

let program_size, max_allocated, total_created, total_deleted = 0

let cache = 0, max_cache = 0, caches = []

const push = (obj) => {
  if (memory_size() === 0) {
    memory[0] = obj
  } else {
    memory[head() + 1] = obj
  }
  ++total_created
  if (memory_size() > max_allocated) {
    max_allocated = memory_size()
  }
}

const pop = (idx = null) => {
  idx = idx || head()
  delete memory[idx]
  ++total_deleted
}

const memory_size = () => Object.keys(memory).length;

const head = () => {
  const keys = Object.keys(memory).map(Number)
  if (keys.length === 0) {
    throw new Error("Can't get head from empty memory")
  }
  return keys[keys.length - 1]
}

const phi_points = []

const add_phi_point = (add, value, scope) => {
  add = USE_PHI_POINTS && add && (phi_points.length === 0 || value > phi_points[phi_points.length - 1])
  if (add) {
    // console.log('add phi', scope, value, value - scope)
    phi_points.push(value)
    if (value - scope > 1) {
      mark_rec(value, scope)
      if (REMOVE_ON_POINTS) {
        update_cache()
        for (let i = scope + 1; i <= value; ++i) {
          if (!!memory[i]) {
            if (!memory[i].stay) {
              pop(i)
            } else {
              memory[i].stay = null
            }
          }
        }
      }
    }
  }
  return add
}

const update_cache = () => {
  caches = []
  if (cache > max_cache) {
    max_cache = cache
  }
  cache = 0
}

const add_disp_point = (from, phi, after = false) => {
  // console.log('add disp', from, phi, phi - from)
  mark_disps(from, from, phi)
  if (after) {
    mark_disps(phi, from, phi)
  }
  update_cache()
  for (let i = from; i <= phi; ++i) {
    if (!!memory[i]) {
      if (!memory[i].stay) {
        pop(i)
      } else {
        memory[i].stay = null
      }
    }
  }
}

const mark_disps = (start, from, to) => {
  memory[start].stay = true
  const tgt = memory[start].target
  Object.keys(tgt).forEach((at) => {
    const atr = tgt[at]
    let ref = null
    if (atr.cache == null && atr.xi != null) {
      ref = atr.xi
    } else if (atr.cache != null) {
      ref = atr.cache
    }
    if (ref != null && ref >= from && ref <= to && !memory[ref].stay) {
      mark_disps(ref, from, to)
    }
  })
}

const mark_rec = (index, scope) => {
  memory[index].stay = true
  const tgt = memory[index].target
  Object.keys(tgt).forEach((at) => {
    const atr = tgt[at]
    let ref = null
    if (atr.cache == null && atr.xi != null) {
      ref = atr.xi
    } else if (atr.cache != null) {
      ref = atr.cache
    }
    if (ref != null && ref > scope && ref < phi_points[phi_points.length - 1] && !memory[ref].stay) {
      mark_rec(ref, scope)
    }
  })
}

const pop_phi_point = (end, scope) => {
  if (USE_PHI_POINTS && end) {
    if (REMOVE_ON_POINTS) {
      const until = phi_points.length > 1 ? phi_points[phi_points.length - 2] : scope
      while (true) {
        if (head() > until) {
          pop()
        } else {
          break
        }
      }
    }
    // console.log('pop phi', phi_points[phi_points.length - 1])
    phi_points.pop()
  }
}

const attr = (value, xi = null, cache = null) => ({value, xi, cache})

// objects
const object = (name, type, target, attr = null, value = null) => ({
  name,
  type,
  target,
  attr,
  value
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
      push({...clone, name: '(copy ' + clone.name + ')', origin: op.target})
      res = head()
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
      res = op.target
      break
  }
  return res
}

const need_contextualize = (index) => {
  const obj = memory[index]
  let need
  switch (obj.type) {
    case FORMATION:
      need = false
      break
    case DISPATCH:
      need = obj.target === -1 || need_contextualize(obj.target)
      break
    case APPLICATION:
      need = obj.target === -1 || obj.value === -1 || need_contextualize(obj.target) || need_contextualize(obj.value)
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
              cache++
              caches.push(tgt_i)
              tgt[obj.attr].cache = at_i
            }
          }

          if (at_i !== 0 && obj.attr !== RHO && !Object.hasOwn(memory[at_i].target, RHO)) {
            res = exec(copy(at_i))
            if (USE_CACHE) {
              cache++
              caches.push(res)
              res = exec(set(res, RHO, attr(tgt_i, null, tgt_i)))
            } else {
              res = exec(set(res, RHO, attr(tgt_i)))
            }
          } else {
            res = at_i
          }
        } else if (Object.hasOwn(tgt, PHI)) {
          push(dispatch(`${tgt_i}.${PHI}`, tgt_i, PHI))
          const phi_i = morph(head(), tgt_i, true)
          add_disp_point(tgt_i, phi_i)
          push(dispatch(`${phi_i}.${obj.attr}`, phi_i, obj.attr))
          res = morph(head(), phi_i, true)
          add_disp_point(tgt_i, res, true)
        } else if (Object.hasOwn(tgt, LAMBDA)) {
          const atom = tgt[LAMBDA].value
          if (!Object.hasOwn(atoms, atom)) {
            throw new Error(`Atom ${atom} does not exist`)
          }
          const atom_res_i = morph(atoms[atom](tgt_i), tgt_i)
          push(dispatch(`${atom_res_i}.${obj.attr}`, atom_res_i, obj.attr))
          res = morph(head(), atom_res_i, true)
          // add_disp_point(tgt_i, res, true)
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
      } else if (need_contextualize(obj.value)) {
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

const dataize = (index, scope = program_size - 1, with_scope = WITH_PHI_POINTS_DEFAULT) => {
  const obj = memory[index]
  let data, started = false
  switch (obj.type) {
    case FORMATION:
      if (Object.hasOwn(obj.target, DELTA)) {
        data = obj.target[DELTA].value
      } else if (Object.hasOwn(obj.target, PHI)) {
        push(dispatch(`${obj.name}.${PHI}`, index, PHI))
        const phi_i = morph(head(), index, true)
        started = add_phi_point(with_scope, phi_i, scope)
        data = dataize(phi_i, scope, with_scope)
      } else if (Object.hasOwn(obj.target, LAMBDA)) {
        const atom = obj.target[LAMBDA].value
        if (!Object.hasOwn(atoms, atom)) {
          throw new Error(`Atom ${atom} does not exist`)
        }
        const atom_res_i = morph(atoms[atom](index), index)
        started = add_phi_point(with_scope, atom_res_i, scope)
        data = dataize(atom_res_i, scope, with_scope)
      } else {
        throw new Error(`Can't dataize object ${index}, no ${DELTA}, no ${PHI}, no ${LAMBDA}`)
      }
      break
    default:
      const op_i = morph(index, index, true)
      data = dataize(op_i, scope, with_scope)
      break
  }
  pop_phi_point(started, scope)
  return data
}

// OBJECTS

program_size = memory_size()
max_allocated = memory_size()
total_created = memory_size()

try {
  const res = bytesOf.bytes(dataize(0, head(), true)).asNumber()
  print_memory()
  console.log(`data: ${res}`)
  console.log(`original program size: ${program_size}`)
  console.log(`total created: ${total_created}`)
  console.log(`total created without program: ${total_created - program_size}`)
  console.log(`total deleted: ${total_deleted}`)
  console.log(`max depth: ${max_allocated}`)
  console.log(`max depth without program: ${max_allocated - program_size}`)
  console.log(`max set cache: ${max_cache}`)
} catch (e) {
  console.log(e)
  print_memory()
  throw e
}