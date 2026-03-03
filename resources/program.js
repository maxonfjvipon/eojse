const {
  print_stack,
  bytesOf,
  REMOVE_UNNECESSARY,
  USE_CACHE,
  COPY_ON_APPLICATION,
  USE_D_SCOPES,
  REMOVE_D_MARKED,
  WITH_SCOPE_DEFAULT,
  HIDE_XI,
  FORMATION,
  APPLICATION,
  DISPATCH,
  COPY,
  SET,
  stack,
  PHI,
  DELTA,
  RHO,
  LAMBDA
} = require('./helpers.js');

let program_size, max_allocated, total_created, total_deleted = 0

const push = (obj) => {
  if (stack_size() === 0) {
    stack[0] = obj
  } else {
    stack[head() + 1] = obj
  }
  ++total_created
  if (stack_size() > max_allocated) {
    max_allocated = stack_size()
  }
}

const pop = (idx = null) => {
  idx = idx || head()
  delete stack[idx]
  ++total_deleted
}

const stack_size = () => Object.keys(stack).length;

const head = () => {
  const keys = Object.keys(stack).map(Number)
  if (keys.length === 0) {
    throw new Error("Can't get head from empty stack")
  }
  return keys[keys.length - 1]
}

const d_scope = []

// limit scope for atoms
// unlimited scope for global dataization
const start_d_scope = (start, value) => {
  const add = USE_D_SCOPES && start && (d_scope.length === 0 || value > d_scope[d_scope.length - 1])
  if (add) {
    // console.log('add scope', value)
    d_scope.push(value)
    if (value - program_size > 1) {
      mark_rec(value)
      if (REMOVE_D_MARKED) {
        for (let i = program_size; i <= value; ++i) {
          if (!!stack[i]) {
            if (!stack[i].stay) {
              pop(i)
            } else {
              stack[i].stay = null
            }
          }
        }
      }
    }
  }
  return add
}

const mark_rec = (index) => {
  stack[index].stay = true
  const tgt = stack[index].target
  Object.keys(tgt).forEach((at) => {
    const atr = tgt[at]
    let ref = null
    if (atr.cache == null && atr.xi != null) {
      ref = atr.xi
    } else if (atr.cache != null) {
      ref = atr.cache
    }
    if (ref != null && ref >= program_size && ref < d_scope[d_scope.length - 1] && !stack[ref].stay) {
      mark_rec(ref)
    }
  })
}



const end_d_scope = (end) => {
  if (USE_D_SCOPES && end) {
    if (REMOVE_D_MARKED) {
      const until = d_scope.length > 1 ? d_scope[d_scope.length - 2] : program_size - 1
      while (true) {
        if (head() > until) {
          pop()
        } else {
          break
        }
      }
    }
    // console.log('pop scope', d_scope[d_scope.length - 1])
    d_scope.pop()
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

    stack[res].from_atom = true

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

    stack[res].from_atom = true

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

    stack[res].from_atom = true

    return res
  },
}

const exec = (op) => {
  let res, tgt
  switch (op.type) {
    case COPY:
      const clone = structuredClone(stack[op.target])
      push({...clone, name: '(copy ' + clone.name + ')', origin: op.target})
      res = head()
      break
    case SET:
      tgt = stack[op.target].target

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
  const obj = stack[index]
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
  const obj = stack[index]
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
      const tgt = stack[tgt_i].target

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
            }
          }

          if (at_i !== 0 && obj.attr !== RHO && !Object.hasOwn(stack[at_i].target, RHO)) {
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
          const phi_i = morph(head(), tgt_i, true)
          push(dispatch(`${phi_i}.${obj.attr}`, phi_i, obj.attr))
          res = morph(head(), phi_i, true)
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
      } else if (need_contextualize(obj.value)) {
        at = attr(obj.value, context)
      } else {
        at = attr(obj.value)
      }

      if (COPY_ON_APPLICATION) {
        tgt_i = exec(copy(tgt_i))
      }

      if (USE_CACHE && stack[at.value].type === FORMATION) {
        at.cache = at.value
      }

      res = exec(set(tgt_i, obj.attr, at))
      break
  }
  return res
}

const dataize = (index, with_scope = WITH_SCOPE_DEFAULT) => {
  const obj = stack[index]
  let data, started = false
  switch (obj.type) {
    case FORMATION:
      if (Object.hasOwn(obj.target, DELTA)) {
        data = obj.target[DELTA].value
      } else if (Object.hasOwn(obj.target, PHI)) {
        push(dispatch(`${obj.name}.${PHI}`, index, PHI))
        const phi_i = morph(head(), index, true)
        started = start_d_scope(with_scope, phi_i)
        data = dataize(phi_i, with_scope)
      } else if (Object.hasOwn(obj.target, LAMBDA)) {
        const atom = obj.target[LAMBDA].value
        if (!Object.hasOwn(atoms, atom)) {
          throw new Error(`Atom ${atom} does not exist`)
        }
        const atom_res_i = morph(atoms[atom](index), index)
        data = dataize(atom_res_i, with_scope)
      } else {
        throw new Error(`Can't dataize object ${index}, no ${DELTA}, no ${PHI}, no ${LAMBDA}`)
      }
      break
    default:
      const op_i = morph(index, index, true)
      data = dataize(op_i, with_scope)
      break
  }
  end_d_scope(started)
  return data
}

// OBJECTS

program_size = stack_size()
max_allocated = stack_size()
total_created = stack_size()

try {
  const res = bytesOf.bytes(dataize(0, true)).asNumber()
  print_stack()
  console.log(`data: ${res}`)
  console.log(`original program size: ${program_size}`)
  console.log(`total created: ${total_created}`)
  console.log(`total created without program: ${total_created - program_size}`)
  console.log(`total deleted: ${total_deleted}`)
  console.log(`max depth: ${max_allocated}`)
  console.log(`max depth without program: ${max_allocated - program_size}`)
} catch (e) {
  console.log(e)
  print_stack()
  throw e
}