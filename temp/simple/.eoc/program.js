const bytesOf = require('./bytes.js');

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

const atoms = {
  'L_number_plus': (self) => {

  }
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
      if (clear) { pop() }

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
          const atom_res_i = morph(atoms[atom](tgt_i), tgt_i)
          push(dispatch(`${atom_res_i}.${obj.attr}`, atom_res_i, obj.attr))
          res = morph(head(), atom_res_i, true)
        } else {
          throw new Error(`Bad dispatch on ${index}, can't go though ${obj.attr}, ${PHI} or ${LAMBDA}`)
        }
      }
      break
    case APPLICATION:
      if (clear) { pop() }

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
        data = dataize(morph(atoms[atom](index), index))
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

push(formation('Φ', {'program': attr(1), 'bytes': attr(8), 'number': attr(10), 'true': attr(28), 'false': attr(34), 'φ': attr(40)})) // 0
push(formation('program', {'x': attr(2), 'φ': attr(7)})) // 1
push(application('x', 3, 0, 4, true)) // 2
push(dispatch('0.number', 0, 'number')) // 3
push(application('5(α0: 6)', 5, 0, 6)) // 4
push(dispatch('0.bytes', 0, 'bytes')) // 5
push(formation('anon', {'Δ': attr(['0x40', '0x45', '0x00', '0x00', '0x00', '0x00', '0x00', '0x00'])})) // 6
push(dispatch('φ', -1, 'x', true)) // 7
push(formation('bytes', {'data': attr(null), 'φ': attr(9)})) // 8
push(dispatch('φ', -1, 'data', true)) // 9
push(formation('number', {'as-bytes': attr(null), 'φ': attr(11), 'plus': attr(12), 'times': attr(13), 'neg': attr(14), 'minus': attr(21), 'gt': attr(27)})) // 10
push(dispatch('φ', -1, 'as-bytes', true)) // 11
push(formation('plus', {'x': attr(null), 'λ': attr('L_number_plus')})) // 12
push(formation('times', {'x': attr(null), 'λ': attr('L_number_times')})) // 13
push(application('neg', 15, 0, 16, true)) // 14
push(dispatch('-1.times', -1, 'times')) // 15
push(application('17(α0: 18)', 17, 0, 18)) // 16
push(dispatch('0.number', 0, 'number')) // 17
push(application('19(α0: 20)', 19, 0, 20)) // 18
push(dispatch('0.bytes', 0, 'bytes')) // 19
push(formation('anon', {'Δ': attr(['0xBF', '0xF0', '0x00', '0x00', '0x00', '0x00', '0x00', '0x00'])})) // 20
push(formation('minus', {'x': attr(null), 'φ': attr(22)})) // 21
push(application('φ', 23, 0, 25, true)) // 22
push(dispatch('24.plus', 24, 'plus')) // 23
push(dispatch('-1.ρ', -1, 'ρ')) // 24
push(dispatch('26.neg', 26, 'neg')) // 25
push(dispatch('-1.x', -1, 'x')) // 26
push(formation('gt', {'x': attr(null), 'λ': attr('L_number_gt')})) // 27
push(formation('true', {'φ': attr(29), 'if': attr(32)})) // 28
push(application('φ', 30, 0, 31, true)) // 29
push(dispatch('0.bytes', 0, 'bytes')) // 30
push(formation('anon', {'Δ': attr(['0x01'])})) // 31
push(formation('if', {'left': attr(null), 'right': attr(null), 'φ': attr(33)})) // 32
push(dispatch('φ', -1, 'left', true)) // 33
push(formation('false', {'φ': attr(35), 'if': attr(38)})) // 34
push(application('φ', 36, 0, 37, true)) // 35
push(dispatch('0.bytes', 0, 'bytes')) // 36
push(formation('anon', {'Δ': attr(['0x00'])})) // 37
push(formation('if', {'left': attr(null), 'right': attr(null), 'φ': attr(39)})) // 38
push(dispatch('φ', -1, 'right', true)) // 39
push(dispatch('φ', 0, 'program', true)) // 40

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