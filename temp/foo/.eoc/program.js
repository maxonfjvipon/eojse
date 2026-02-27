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

push(formation('Φ', {'program': attr(1), 'bytes': attr(14), 'number': attr(16), 'true': attr(34), 'false': attr(40), 'φ': attr(46)})) // 0
push(formation('program', {'foo': attr(2), 'x': attr(4), 'φ': attr(9)})) // 1
push(formation('foo', {'y': attr(null), 'φ': attr(3)})) // 2
push(dispatch('φ', -1, 'y', true)) // 3
push(application('x', 5, 0, 6, true)) // 4
push(dispatch('0.number', 0, 'number')) // 5
push(application('7(α0: 8)', 7, 0, 8)) // 6
push(dispatch('0.bytes', 0, 'bytes')) // 7
push(formation('anon', {'Δ': attr(['0x40', '0x41', '0x80', '0x00', '0x00', '0x00', '0x00', '0x00'])})) // 8
push(application('φ', 10, 0, 11, true)) // 9
push(dispatch('-1.foo', -1, 'foo')) // 10
push(application('12(α0: 13)', 12, 0, 13)) // 11
push(dispatch('-1.foo', -1, 'foo')) // 12
push(dispatch('-1.x', -1, 'x')) // 13
push(formation('bytes', {'data': attr(null), 'φ': attr(15)})) // 14
push(dispatch('φ', -1, 'data', true)) // 15
push(formation('number', {'as-bytes': attr(null), 'φ': attr(17), 'plus': attr(18), 'times': attr(19), 'neg': attr(20), 'minus': attr(27), 'gt': attr(33)})) // 16
push(dispatch('φ', -1, 'as-bytes', true)) // 17
push(formation('plus', {'x': attr(null), 'λ': attr('L_number_plus')})) // 18
push(formation('times', {'x': attr(null), 'λ': attr('L_number_times')})) // 19
push(application('neg', 21, 0, 22, true)) // 20
push(dispatch('-1.times', -1, 'times')) // 21
push(application('23(α0: 24)', 23, 0, 24)) // 22
push(dispatch('0.number', 0, 'number')) // 23
push(application('25(α0: 26)', 25, 0, 26)) // 24
push(dispatch('0.bytes', 0, 'bytes')) // 25
push(formation('anon', {'Δ': attr(['0xBF', '0xF0', '0x00', '0x00', '0x00', '0x00', '0x00', '0x00'])})) // 26
push(formation('minus', {'x': attr(null), 'φ': attr(28)})) // 27
push(application('φ', 29, 0, 31, true)) // 28
push(dispatch('30.plus', 30, 'plus')) // 29
push(dispatch('-1.ρ', -1, 'ρ')) // 30
push(dispatch('32.neg', 32, 'neg')) // 31
push(dispatch('-1.x', -1, 'x')) // 32
push(formation('gt', {'x': attr(null), 'λ': attr('L_number_gt')})) // 33
push(formation('true', {'φ': attr(35), 'if': attr(38)})) // 34
push(application('φ', 36, 0, 37, true)) // 35
push(dispatch('0.bytes', 0, 'bytes')) // 36
push(formation('anon', {'Δ': attr(['0x01'])})) // 37
push(formation('if', {'left': attr(null), 'right': attr(null), 'φ': attr(39)})) // 38
push(dispatch('φ', -1, 'left', true)) // 39
push(formation('false', {'φ': attr(41), 'if': attr(44)})) // 40
push(application('φ', 42, 0, 43, true)) // 41
push(dispatch('0.bytes', 0, 'bytes')) // 42
push(formation('anon', {'Δ': attr(['0x00'])})) // 43
push(formation('if', {'left': attr(null), 'right': attr(null), 'φ': attr(45)})) // 44
push(dispatch('φ', -1, 'right', true)) // 45
push(dispatch('φ', 0, 'program', true)) // 46

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