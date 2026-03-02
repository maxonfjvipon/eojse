const bytesOf = require('./bytes.js')

const stack = {}

const push = (obj) => {
  if (stack_size() === 0) {
    stack[0] = obj
  } else {
    stack[head() + 1] = obj
  }
}

const pop = () => {
  delete stack[head()]
}

const stack_size = () => Object.keys(stack).length;

const head = () => {
  const keys = Object.keys(stack).map(Number)
  if (keys.length === 0) {
    throw new Error("Can't get head from empty stack")
  }
  return keys[keys.length - 1]
}

const scope = []

const start_scope = (start, value, attr) => {
  const add = USE_SCOPES && start && (scope.length === 0 || value > scope[scope.length - 1])
  if (add) {
    console.log('add scope', value)
    scope.push(value)
    mark_objects()
  }
  return add
}

const end_scope = (end) => {
  if (USE_SCOPES && end) {
    console.log('pop scope', scope[scope.length - 1])
    scope.pop()
  }
}

const FORMATION = "FRM", DISPATCH = "DSP", APPLICATION = "APP", COPY = "CPY", SET = "SET"

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

const PHI = 'φ'
const DELTA = 'Δ'
const RHO = 'ρ'
const XI = 'ξ'
const LAMBDA = 'λ'

const REMOVE_UNNECESSARY = true
const USE_CACHE = true
const COPY_ON_APPLICATION = false
const USE_SCOPES = true
const REMOVE_MARKED = false
const WITH_SCOPE_DEFAULT = false
const HIDE_XI = false

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

const print_object = (index) => {
  const obj = stack[index]
  let res
  const form = (o) => {
    let r = '{'
    Object.keys(o).forEach((at, idx) => {
      if (idx > 0) {
        r += ', '
      }
      r += `'${at}':`
      if (at === LAMBDA || o[at].value == null) {
        r += `'${o[at].value}'`
      } else if (at === DELTA) {
        r += `[${o[at].value}]`
      } else if (o[at].xi == null && o[at].cache == null) {
        r += o[at].value
      } else if (o[at].value === o[at].cache) {
        r += `${o[at].value}!`
      } else {
        r += '{' + [o[at].value, HIDE_XI ? '*' : o[at].xi, o[at].cache].join(', ') + '}'
      }
    })
    r += '}'
    return r
  }
  switch (obj.type) {
    case FORMATION:
      res = [
        `${index}: ${form(obj.target)}`,
        // `${idx}: ${JSON.stringify(obj.target)}`,
        Object.hasOwn(obj.target, DELTA) ? ' (DATA ' + bytesOf.bytes(obj.target[DELTA].value).verbose() + ')' : '',
        Object.hasOwn(obj, 'stay') ? ' (STAY)' : '',
        Object.hasOwn(obj, 'from_atom') ? ' (FROM ATOM)' : '',
      ].join('')
      break
    case DISPATCH:
      res = [
        `${index}: `,
        `${obj.target}.${obj.attr}`,
      ].join('')
      break
    case APPLICATION:
      res = [
        `${index}: `,
        `${obj.target}(${obj.attr}: ${obj.value})`,
      ].join('')
      break
  }
  return res + ' // ' + obj.name
}

const print_stack = () => {
  Object.keys(stack).map(Number).forEach((idx) => {
    console.log(print_object(idx))
  })
}

const mark_objects = () => {
  if (scope.length > 1) {
    const pre_last = scope[scope.length - 2]
    const last = scope[scope.length - 1]
    mark_rec(pre_last, pre_last, true)
    mark_rec(last, last, false)

    if (REMOVE_MARKED) {
      for (let i = pre_last + 1; i < last; ++i) {
        if (!!stack[i] && !stack[i].stay) {
          delete stack[i]
        }
      }
    }
  }
}

const mark_rec = (index, border, more) => {
  stack[index].stay = true
  const tgt = stack[index].target
  Object.keys(tgt).forEach((at) => {
    const atr = tgt[at]
    if (!!atr.cache && !stack[atr.cache].stay) {
      if (more && atr.cache > border) {
        mark_rec(atr.cache, border, more)
      } else if (!more && atr.cache < border) {
        mark_rec(atr.cache, border, more)
      }
    }
  })
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
          const atom_res_i = morph(atoms[atom](tgt_i, dataize, morph), tgt_i)
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
        started = start_scope(with_scope, phi_i, PHI)
        data = dataize(phi_i, with_scope)
      } else if (Object.hasOwn(obj.target, LAMBDA)) {
        const atom = obj.target[LAMBDA].value
        if (!Object.hasOwn(atoms, atom)) {
          throw new Error(`Atom ${atom} does not exist`)
        }
        const atom_res_i = morph(atoms[atom](index), index)
        // started = start_scope(with_scope, atom_res_i, LAMBDA)
        data = dataize(atom_res_i, with_scope)
      } else {
        throw new Error(`Can't dataize object ${index}, no ${DELTA}, no ${PHI}, no ${LAMBDA}`)
      }
      break
    default:
      const op_i = morph(index, index, true)
      // started = start_scope(with_scope, op_i, '?') // todo
      data = dataize(op_i, with_scope)
      break
  }
  end_scope(started)
  return data
}

// OBJECTS

const before = stack_size()

try {
  const res = bytesOf.bytes(dataize(0, true)).asNumber()
  print_stack()
  console.log(`data: ${res}`)
  console.log(`total: ${stack_size()}`)
  console.log(`in runtime: ${stack_size() - before}`)
  console.log(`to delete: ${Object.keys(stack).filter((key) => key > before && !stack[key].stay).length}`)
} catch (e) {
  console.log(e)
  print_stack()
  throw e
}