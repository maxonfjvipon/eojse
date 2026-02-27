const bytesOf = require('./bytes.js')

const stack = []

const push = (obj) => {
  stack.push(obj)
}

const pop = () => {
  stack.pop()
}

const head = () => stack.length - 1

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
        `${index}: ${form(obj.target)}`,
        // `${idx}: ${JSON.stringify(obj.target)}`,
        Object.hasOwn(obj.target, DELTA) ? ' (DATA ' + bytesOf.bytes(obj.target[DELTA].value).verbose() + ')' : '',
        Object.hasOwn(obj, 'from_atom') ? ' (FROM ATOM)' : '',
        // Object.hasOwn(obj, 'stay') ? ' (STAY)' : ''
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
  stack.forEach((_, idx) => {
    console.log(print_object(idx))
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
          at = tgt[obj.attr]

          let ctx
          if (at.xi != null) {
            ctx = at.xi
          } else {
            ctx = tgt_i
          }
          const at_i = morph(at.value, ctx)

          if (at_i !== 0 && !Object.hasOwn(stack[at_i].target, RHO)) {
            res = exec(copy(at_i))
            res = exec(set(res, RHO, attr(tgt_i)))
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

      res = exec(set(tgt_i, obj.attr, at))
      break
  }
  return res
}

const dataize = (index) => {
  const obj = stack[index]
  let data
  switch (obj.type) {
    case FORMATION:
      if (Object.hasOwn(obj.target, DELTA)) {
        data = obj.target[DELTA].value
      } else if (Object.hasOwn(obj.target, PHI)) {
        push(dispatch(`${obj.name}.${PHI}`, index, PHI))
        const phi_i = morph(head(), index, true)
        data = dataize(phi_i)
      } else if (Object.hasOwn(obj.target, LAMBDA)) {
        const atom = obj.target[LAMBDA].value
        if (!Object.hasOwn(atoms, atom)) {
          throw new Error(`Atom ${atom} does not exist`)
        }
        data = dataize(morph(atoms[atom](index, dataize, morph), index))
      } else {
        throw new Error(`Can't dataize object ${index}, no ${DELTA}, no ${PHI}, no ${LAMBDA}`)
      }
      break
    default:
      data = dataize(morph(index, index, true))
      break
  }
  return data
}

// OBJECTS

try {
  const res = bytesOf.bytes(dataize(0)).asNumber()
  print_stack()
  console.log(`data: ${res}`)
  console.log(`total: ${stack.length}`)
} catch (e) {
  console.log(e)
  print_stack()
  throw e
}