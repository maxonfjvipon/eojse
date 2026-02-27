const bytesOf = require('./bytes.js');

// STACK
const stack = {}
let stackLength = 0
const scope = []

let GLOBAL = 0

const start_scope = (flag, id, through) => {
  const add = USE_SCOPES && flag && (scope.length == 0 || id > scope[scope.length - 1])
  if (add) {
    console.log('push scope', id, through)
    scope.push(id)
    mark_objects()
  }
  return add
}

const end_scope = (flag) => {
  if (USE_SCOPES && flag) {
    console.log('pop scope', scope[scope.length - 1])
    scope.pop()
  }
}

const push = (obj) => {
  stack[stackLength] = obj
  if (SEQUENCE) {
    console.log('push', head(), print_object(head()))
  }
  stackLength++
}

const pop = () => {
  delete stack[stackLength - 1]
  stackLength--
  if (SEQUENCE) {
    console.log('pop')
  }
}

const head = () => stackLength - 1

const PHI = 'φ'
const DELTA = 'Δ'
const RHO = 'ρ'
const XI = 'ξ'
const LAMBDA = 'λ'
const USE_CACHE = false
const REMOVE_UNNECESSARY = true
const CLEAR_ATOMS = false
const SEQUENCE = false
const COPY_ON_APPLICAION = false
const CLEAR_CACHE_ON_COPY = false
const CLEAR_REF_ON_COPY = true
const LOG_ATOMS = false
const REMOVE_MARKED = true
const USE_SCOPES = false

const FORMATION = "FRM", DISPATCH = "DSP", APPLICATION = "APP", TAKE = "TKE", COPY = "CPY", SET = "SET"

const attr = (value, xi = null, cache = null) => ({ value, xi, cache })

// objects
const object = (name, type, target, attr = null, value = null) => ({ name, type, target, attr, value })

const formation = (name, attrs) => object(name, FORMATION, attrs)

const dispatch = (name, target, attr, cache = false) => object(name, DISPATCH, target, attr, null)

const application = (name, target, attr, value, cache = false) => object(name, APPLICATION, target, attr, value)

// OPERATIONS
const operation = (type, target, attr = null, value = null) => ({ type, target, attr, value })

const take = (target, attr) => operation(TAKE, target, attr)

const copy = (target) => operation(COPY, target)

const set = (target, attr, value) => operation(SET, target, attr, value)

const print_object = (index) => {
  let obj, idx
  if (typeof index == 'number') {
    obj = stack[index]
    idx = index
  } else {
    obj = index
    idx = 'unknown'
  }
  if (obj === undefined) {
    throw new Error('Object by index ' + idx + ' can not be printed')
  }
  // const ref = `${USE_CACHE && !!obj.ref ? ' (REF: ' + JSON.stringify(obj.ref) + ')' : ''}`
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
      } else if (o[at].value == o[at].cache) {
        r += o[at].value
      } else {
        r += '{' + [o[at].value, o[at].xi, o[at].cache].join(', ') + '}'
      }
    })
    r += '}'
    return r
  }
  switch (obj.type) {
    case FORMATION:
      res = [
        `${idx}: ${form(obj.target)}`,
        // `${idx}: ${JSON.stringify(obj.target)}`,
        Object.hasOwn(obj.target, DELTA) ? ' (DATA ' + bytesOf.bytes(obj.target[DELTA].value).verbose() + ')' : '',
        // ref,
        Object.hasOwn(obj, 'from_atom') ? ' (FROM ATOM)' : '',
        Object.hasOwn(obj, 'stay') ? ' (STAY)' : ''
      ].join('')
      break
    case DISPATCH:
      res = [
        `${idx}: `,
        `${obj.target}.${obj.attr}`,
      ].join('')
      break
    case APPLICATION:
      res = [
        `${idx}: `,
        `${obj.target}(${obj.attr}: ${obj.value})`,
      ].join('')
      break
  }
  return res + ' // ' + obj.name
}

/**
 * Print stack state.
 */
const print_stack = (from = null) => {
  if (from != null) {
    console.log('-----------------')
  }
  // Convert stack object to array and sort by index for sequential printing
  const indices = Object.keys(stack).map(Number).sort((a, b) => a - b)
  indices.forEach(idx => {
    if (from == null || idx > from) {
      console.log(print_object(idx))
    }
  })
}

/**
 * Hex byte array to int byte array.
 * Converts this:
 * [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x39]
 * to this:
 * [0, 0, 0, 0, 0, 0, 48, 57]
 */

/**
 * Executes operation and returns adress to object
 * @param {Object} op - operation to execute
 * @returns Adress to object after executed operation
 */
const exec = (op) => {
  let res, tgt
  switch (op.type) {
    case COPY:
      tgt = stack[op.target]
      const clone = structuredClone(tgt)
      if (CLEAR_REF_ON_COPY && clone.ref) {
        clone.ref = null
      }
      if (CLEAR_CACHE_ON_COPY) {
        Object.keys(clone.target).forEach((at) => {
          clone.target[at].cache = null
        })
      }
      push({ ...clone, name: "copy " + clone.name, origin: op.target })
      res = head()
      break
    case SET:
      tgt = stack[op.target].target
      let at
      if (typeof op.attr == 'string') {
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

/**
 * Evaluate object to formation and returns adress to it
 * @param {Number} index - address of object
 * @param {Number} xi - address of formation we're in
 * @returns adress of morphed formation
 */
const morph = (index, xi, remove = false) => {
  let res, tgt_i, at_i, cpy_i
  const obj = stack[index]
  const _pop = REMOVE_UNNECESSARY && remove
  switch (obj.type) {
    case FORMATION:
      res = index
      break
    case DISPATCH:
      if (_pop) { pop() }

      if (obj.target == -1) {
        tgt_i = xi
      } else {
        tgt_i = morph(obj.target, xi)
      }
      const tgt = stack[tgt_i].target

      if (obj.attr === '-1') {
        res = xi; // special case for $ > name
      } else {
        if (Object.hasOwn(tgt, obj.attr)) {
          if (USE_CACHE && tgt[obj.attr].cache != null) {
            at_i = tgt[obj.attr].cache
          } else {
            const taken = tgt[obj.attr]
            let _xi
            if (taken.xi != null) {
              _xi = taken.xi
            } else {
              _xi = tgt_i
            }
            at_i = morph(taken.value, _xi)

            if (USE_CACHE && tgt[obj.attr].cache == null) {
              tgt[obj.attr].cache = at_i
            }
          }

          if (at_i !== 0 && !Object.hasOwn(stack[at_i].target, RHO)) {
            cpy_i = exec(copy(at_i))
            if (USE_CACHE) {
              cpy_i = exec(set(cpy_i, RHO, attr(tgt_i, null, tgt_i)))
            } else {
              cpy_i = exec(set(cpy_i, RHO, attr(tgt_i)))
            }
            res = cpy_i
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
        }
      }
      break
    case APPLICATION:
      if (_pop) { pop() }

      tgt_i = morph(obj.target, xi)

      let at
      if (obj.value == -1) {
        at = attr(xi)
      } else if (need_contextualize(obj.value, xi)) {
        // set up new context
        at = attr(obj.value, xi)
      } else {
        at = attr(obj.value)
      }

      // set cache right in place if applied object is formation
      if (USE_CACHE && stack[at.value].type === FORMATION) {
        at.cache = at.value
      }

      if (COPY_ON_APPLICAION) {
        tgt_i = exec(copy(tgt_i))
      }

      res = exec(set(tgt_i, obj.attr, at))
      break
  }
  return res
}

/**
 * Check if it's needed to resolve context for application argument
 * @param {Number} index - address of object for resolving
 * @returns {Boolean}
 */
const need_contextualize = (index) => {
  const obj = stack[index]
  let need
  switch (obj.type) {
    case FORMATION:
      need = false
      break
    case DISPATCH:
      need = obj.target == -1 || need_contextualize(obj.target)
      break
    case APPLICATION:
      need = obj.target == -1 || obj.value == -1 || need_contextualize(obj.target) || need_contextualize(obj.value)
      break
  }
  return need
}

const dataize = (index, with_scope = false) => {
  const obj = stack[index]
  let data, next, added
  switch (obj.type) {
    case FORMATION:
      if (Object.hasOwn(obj.target, DELTA)) {
        data = obj.target[DELTA].value
      } else if (Object.hasOwn(obj.target, PHI)) {
        push(dispatch(`${obj.name}.${PHI}`, index, PHI))
        next = morph(head(), index, true)
        added = start_scope(with_scope, next, PHI)
        data = dataize(next, with_scope)
      } else if (Object.hasOwn(obj.target, LAMBDA)) {
        const atom = obj.target[LAMBDA].value
        if (!Object.hasOwn(atoms, atom)) {
          throw new Error(`Atom ${atom} does not exist`)
        }
        next = morph(atoms[atom](index), index)
        added = start_scope(with_scope, next, LAMBDA)
        data = dataize(next, with_scope)
      } else {
        throw new Error(`Can't dataize, no ${DELTA}, no ${PHI}`)
      }
      break
    default:
      next = morph(index, index, true)
      // start_scope(with_scope, next) // todo ?
      data = dataize(next, with_scope)
      break;
  }
  end_scope(with_scope && added)
  return data
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

const atoms = {
  'L_number_plus': (self) => {
    if (SEQUENCE) {
      console.log('atom PLUS starts')
    }
    push(dispatch(`${self}.${RHO}`, self, RHO))
    const left = bytesOf.bytes(dataize(head())).asNumber()

    push(dispatch(`${self}.x`, self, 'x'))
    const right = bytesOf.bytes(dataize(head())).asNumber()

    const res = left + right
    push(dispatch('0.number', 0, 'number'))
    const num = morph(head(), self, true)
    push(dispatch('0.bytes', 0, 'bytes'))
    const bts = morph(head(), self, true)
    push(formation('plus res', { 'Δ': attr(bytesOf.number(res).asBytes()) }))
    const data = morph(head(), self, true)
    push(application(`${bts}(0: ${data})`, bts, 0, data))
    const _bts = morph(head(), self, true)
    push(application(`${num}(0: ${_bts})`, num, 0, _bts))
    const _res = morph(head(), self, true)

    stack[_res].from_atom = true

    if (SEQUENCE) {
      console.log('atom PLUS ends')
    }

    return _res
  },
  'L_number_times': (self) => {
    const before = head()

    push(dispatch(`${self}.${RHO}`, self, RHO))
    const rho = morph(head(), self, true)
    push(dispatch(`${self}.x`, self, 'x'))
    const x = morph(head(), self, true)
    const left = bytesOf.bytes(dataize(rho)).asNumber()
    const right = bytesOf.bytes(dataize(x)).asNumber()

    const clear = head()

    const res = left * right
    push(dispatch('0.number', 0, 'number'))
    const num = morph(head(), self, true)
    push(dispatch('0.bytes', 0, 'bytes'))
    const bts = morph(head(), self, true)
    push(formation('plus res', { 'Δ': attr(bytesOf.number(res).asBytes()) }))
    const data = morph(head(), self, true)
    push(application(`${bts}(0: ${data})`, bts, 0, data))
    const _bts = morph(head(), self, true)
    push(application(`${num}(0: ${_bts})`, num, 0, _bts))
    const _res = morph(head(), self, true)

    if (!CLEAR_ATOMS && LOG_ATOMS) {
      console.log('!!atom L_number_times returns ' + _res + ', need to clear in atom from ' + (before + 1) + ' to ' + clear + ' (including)!!')
    }

    stack[_res].from_atom = true

    return _res
  },
  'L_number_gt': (self) => {
    if (SEQUENCE) {
      console.log('atom GT starts')
    }

    const before = head()

    push(dispatch(`${self}.${RHO}`, self, RHO))
    const rho = morph(head(), self, true)
    push(dispatch(`${self}.x`, self, 'x'))
    const x = morph(head(), self, true)
    const left = bytesOf.bytes(dataize(rho)).asNumber()
    const right = bytesOf.bytes(dataize(x)).asNumber()

    const clear = head()

    const bool = left > right ? 'true' : 'false'
    push(dispatch('0.' + bool, 0, bool))
    const res = morph(head(), self, true)

    if (!CLEAR_ATOMS && LOG_ATOMS) {
      console.log('!!atom L_number_gt ' + before + ' and returns ' + res + ', need to clear in atom from ' + (before + 1) + ' to ' + clear + ' (including)!!')
    }

    stack[res].from_atom = true

    if (SEQUENCE) {
      console.log('atom GT ends')
    }

    return res
  },
  'L_random': (self) => {

    push(dispatch('0.number', 0, 'number'))
    const num = morph(head(), self, true)
    push(dispatch('0.bytes', 0, 'bytes'))
    const bts = morph(head(), self, true)
    push(formation('plus res', { 'Δ': attr(bytesOf.number(++GLOBAL).asBytes()) }))
    const data = morph(head(), self, true)
    push(application(`${bts}(0: ${data})`, bts, 0, data))
    const _bts = morph(head(), self, true)
    push(application(`${num}(0: ${_bts})`, num, 0, _bts))
    const _res = morph(head(), self, true)

    stack[_res].from_atom = true

    return _res
  },
  'L_plus': (self) => {

    push(dispatch(`${self}.x`, self, 'x'))
    const x = morph(head(), self, true)
    dataize(x)

    push(dispatch(`${self}.y`, self, 'y'))
    const y = morph(head(), self, true)
    dataize(y)

    push(dispatch('0.number', 0, 'number'))
    const num = morph(head(), self, true)
    push(dispatch('0.bytes', 0, 'bytes'))
    const bts = morph(head(), self, true)
    push(formation('L_plus res', { 'Δ': attr(bytesOf.number(55).asBytes()) }))
    const data = morph(head(), self, true)
    push(application(`${bts}(0: ${data})`, bts, 0, data))
    const _bts = morph(head(), self, true)
    push(application(`${num}(0: ${_bts})`, num, 0, _bts))
    const _res = morph(head(), self, true)

    stack[_res].from_atom = true

    return _res
  }
}

// OBJECTS

const from = head()
if (!SEQUENCE && !USE_CACHE) {
  print_stack()
}

try {
  const res = bytesOf.bytes(dataize(0, true)).asNumber()

  if (!SEQUENCE) {
    if (!USE_CACHE) {
      print_stack(from)
    } else {
      print_stack()
    }
  }
  console.log(`data: ${res}`)
} catch (e) {
  console.log(e)
  if (!SEQUENCE) {
    if (!USE_CACHE) {
      print_stack(from)
    } else {
      print_stack()
    }
  }
}

console.log('TOTAL', Object.keys(stack).length)
// console.log(Object.keys(stack).filter((idx) => stack[idx].stay).length + 68)
